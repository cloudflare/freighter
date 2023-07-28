use freighter_api_types::index::request::Publish;
use freighter_api_types::index::response::{
    CompletedPublication, CrateVersion, ListAll, RegistryConfig,
};
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::{Body, Request, StatusCode};
use semver::Version;
use thiserror::Error;

const API_PATH: &str = "api/v1/crates";

pub struct Client {
    http: reqwest::Client,
    endpoint: String,
    token: Option<String>,
    config: RegistryConfig,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Received error from freighter server: {0}")]
    ServerError(#[source] anyhow::Error),
    #[error("Conflict due to resource already being present")]
    Conflict,
    #[error("Permission denied to perform operation")]
    Unauthorized,
    #[error("Requested object was not found")]
    NotFound,
    #[error("Failed to deserialize stuff")]
    Deserialization(#[from] serde_json::Error),
    #[error("Received unknown error")]
    Other(#[from] anyhow::Error),
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        if let Some(status) = value.status() {
            match status {
                StatusCode::INTERNAL_SERVER_ERROR => Self::ServerError(anyhow::anyhow!(value)),
                StatusCode::CONFLICT => Self::Conflict,
                StatusCode::UNAUTHORIZED => Self::Unauthorized,
                StatusCode::NOT_FOUND => Self::NotFound,
                _ => Self::Other(anyhow::anyhow!(value)),
            }
        } else {
            Self::Other(anyhow::anyhow!(value))
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

impl Client {
    pub async fn new(endpoint: &str) -> Self {
        let http = reqwest::Client::new();

        Self::from_reqwest(endpoint, http).await
    }

    pub async fn from_reqwest(endpoint: &str, client: reqwest::Client) -> Self {
        let endpoint = endpoint.to_string();
        let config_url = format!("{endpoint}/config.json");

        let resp = client.get(config_url).send().await.unwrap();

        let mut config: RegistryConfig = resp.json().await.unwrap();

        if config.api.ends_with('/') {
            config.api.pop();
        }

        if config.dl.ends_with('/') {
            config.dl.pop();
        }

        Self {
            http: client,
            endpoint,
            token: None,
            config,
        }
    }

    pub async fn fetch_index(&self, name: &str) -> Result<Vec<CrateVersion>> {
        let prefix = match name.len() {
            0 => panic!("Should not be asked for crate name of len 0"),
            1 => "1".to_string(),
            2 => "2".to_string(),
            3 => format!("3/{}", name.split_at(1).0),
            _ => {
                let (prefix_1_tmp, rest) = name.split_at(2);
                let (prefix_2_tmp, _) = rest.split_at(2);
                format!("{prefix_1_tmp}/{prefix_2_tmp}")
            }
        };

        let url = format!("{}/{prefix}/{name}", &self.endpoint);

        let mut req = self.http.get(url).build().unwrap();

        self.attach_auth(&mut req);

        let resp = self.http.execute(req).await?;

        resp.error_for_status_ref()?;

        let text = resp.text().await?;

        let mut crates = Vec::new();

        for l in text.lines() {
            crates.push(serde_json::from_str(l)?);
        }

        Ok(crates)
    }

    pub async fn download_crate(&self, name: &str, version: &Version) -> Result<Vec<u8>> {
        let url = format!("{}/{name}/{version}", self.config.dl);

        let mut req = self.http.get(url).build().unwrap();

        self.attach_auth(&mut req);

        let resp = self.http.execute(req).await?;

        resp.error_for_status_ref()?;

        let bytes = resp.bytes().await?;

        Ok(bytes.to_vec())
    }

    pub async fn publish(&self, version: &Publish, tarball: &[u8]) -> Result<CompletedPublication> {
        let serialized = serde_json::to_vec(version)?;

        let tarball_len_off = 4 + serialized.len();
        let tarball_off = 4 + tarball_len_off;

        let mut buf = vec![0; tarball_off + tarball.len()];

        // copy json len to buffer
        buf[0..4].copy_from_slice(&(serialized.len() as u32).to_le_bytes());

        // copy json to buffer
        buf[4..tarball_len_off].copy_from_slice(&serialized);

        // copy tarball len to buffer
        buf[tarball_len_off..tarball_off].copy_from_slice(&(tarball.len() as u32).to_le_bytes());

        // copy tarball to buffer
        buf[tarball_off..].copy_from_slice(tarball);

        let url = format!("{}/{API_PATH}/new", &self.config.api);

        let mut req = self.http.put(url).build().unwrap();

        *req.body_mut() = Some(Body::from(buf));

        self.attach_auth(&mut req);

        let resp = self.http.execute(req).await?;

        resp.error_for_status_ref()?;

        let json = resp.json().await?;

        Ok(json)
    }

    pub async fn list(&self, per_page: Option<usize>, page: Option<usize>) -> Result<ListAll> {
        let url = format!("{}/all", self.config.api);

        let mut req = self.http.get(url).build().unwrap();

        self.attach_auth(&mut req);

        {
            let mut query_pairs = req.url_mut().query_pairs_mut();

            if let Some(inner) = per_page {
                query_pairs.append_pair("per_page", &inner.to_string());
            }

            if let Some(inner) = page {
                query_pairs.append_pair("page", &inner.to_string());
            }
        }

        let resp = self.http.execute(req).await?;

        resp.error_for_status_ref()?;

        let json = resp.json().await?;

        Ok(json)
    }

    // pub async fn search(&self, query: &str, per_page: Option<usize>) -> Result<SearchResults> {
    //     todo!()
    // }

    // pub async fn yank(&self, name: &str, version: &Version) {
    //     todo!()
    // }
    //
    // pub async fn unyank(&self, name: &str, version: &Version) {
    //     todo!()
    // }
    //
    // pub async fn list_owners(&self, name: &str) {
    //     todo!()
    // }
    //
    // pub async fn add_owners(&self, name: &str, owners: &[&str]) {
    //     todo!()
    // }
    //
    // pub async fn remove_owners(&self, name: &str, owners: &[&str]) {
    //     todo!()
    // }

    pub async fn register(&mut self, username: &str, password: &str) -> Result<()> {
        let url = format!("{}/{API_PATH}/account", self.config.api);

        let mut req = self
            .http
            .post(url)
            .form(&[("username", username), ("password", password)])
            .build()
            .unwrap();

        self.attach_auth(&mut req);

        let resp = self.http.execute(req).await?;

        resp.error_for_status_ref()?;

        let text = resp.text().await?;

        self.token = Some(text);

        Ok(())
    }

    pub async fn login(&mut self, username: &str, password: &str) -> Result<()> {
        let url = format!("{}/{API_PATH}/account/token", self.config.api);

        let mut req = self
            .http
            .post(url)
            .form(&[("username", username), ("password", password)])
            .build()
            .unwrap();

        self.attach_auth(&mut req);

        let resp = self.http.execute(req).await?;

        resp.error_for_status_ref()?;

        let text = resp.text().await?;

        self.token = Some(text);

        Ok(())
    }

    pub async fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_ref().map(String::as_str)
    }

    fn attach_auth(&self, req: &mut Request) {
        if let Some(token) = self.token.as_ref() {
            req.headers_mut()
                .append(AUTHORIZATION, HeaderValue::from_str(token).unwrap());
        }
    }
}
