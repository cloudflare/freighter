use crate::cf_access::{CfAccess, UserId};
use crate::{AuthError, AuthProvider, AuthResult};
use async_trait::async_trait;
use freighter_api_types::ownership::response::ListedOwner;
use http::{header, HeaderMap, StatusCode};
use serde::Deserialize;
use std::collections::HashSet;

/// Registry auth based on Cloudflare Access, using `JsonWebTokens` for auth
///
/// It's not possible to validate Service Auth tokens direclty. To use a token,
/// you need to log in with it to an Access-protected URL, and obtain the JWT from
/// the `CF_Authorization` cookie. This temporary cookie is the only way
/// auth with this Freighter backend.
///
/// For personal account, you can call `cloudflared access token` to obtain the JWT.
pub struct CfAuthProvider {
    team_base_url: String,
    access: CfAccess,
    publish_access_ids: HashSet<String>,
}

impl CfAuthProvider {
    pub fn new(config: Config) -> AuthResult<Self> {
        let access = CfAccess::new(&config.auth_team_base_url, &config.auth_audience)
            .map_err(AuthError::ServiceError)?;
        Ok(Self {
            access,
            team_base_url: config.auth_team_base_url,
            publish_access_ids: config.auth_publish_access_ids,
        })
    }

    async fn validated_user_id(&self, token: &str) -> AuthResult<UserId> {
        self.access.validated_user_id(token).await
    }

    async fn validated_non_service_user_id(&self, token: &str) -> AuthResult<UserId> {
        let id = self.validated_user_id(token).await?;
        Self::authorize_non_service_mutation(&id)?;
        Ok(id)
    }

    fn authorize_non_service_mutation(id: &UserId) -> AuthResult<()> {
        if id.is_service_token() {
            Err(AuthError::Forbidden)
        } else {
            Ok(())
        }
    }

    fn authorize_publish(&self, id: &UserId) -> AuthResult<()> {
        if id.is_service_token() && self.publish_access_ids.contains(&id.0) {
            Ok(())
        } else {
            Err(AuthError::Forbidden)
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct Config {
    /// `https://<team name>.cloudflareaccess.com`
    #[serde(default = "default_auth_team_base_url")]
    pub auth_team_base_url: String,
    /// Long hash from overview tab
    #[serde(default = "default_auth_auth_audience")]
    pub auth_audience: String,
    /// List of token IDs ("xxx.access") allowed to publish crates
    #[serde(default = "default_auth_publish_access_ids")]
    pub auth_publish_access_ids: HashSet<String>,
}

fn default_auth_team_base_url() -> String {
    std::env::var("FREIGHTER_AUTH_TEAM_BASE_URL")
        .expect("auth_team_base_url not found in config or environment")
}

fn default_auth_auth_audience() -> String {
    std::env::var("FREIGHTER_AUTH_AUDIENCE")
        .expect("auth_audience not found in config or environment")
}

fn default_auth_publish_access_ids() -> HashSet<String> {
    std::env::var("FREIGHTER_AUTH_PUBLISH_ACCESS_IDS")
        .expect("auth_publish_access_ids not found in config or environment")
        .split([',', ':', ';'])
        .map(String::from)
        .collect()
}

#[async_trait]
impl AuthProvider for CfAuthProvider {
    type Config = Config;

    async fn healthcheck(&self) -> anyhow::Result<()> {
        self.access.refresh().await?;
        Ok(())
    }

    async fn register(&self, _username: &str) -> AuthResult<String> {
        Err(AuthError::Unimplemented)
    }

    fn register_supported(&self) -> Result<(), &'static str> {
        Err("<h1>Registration is only via <code>cloudflared</code></h1>
<style>var{color:red}</style>
<p>Run:</p>
<pre>
cloudflared access login <var>hostname of the registry</var> | fgrep . | cargo login --registry=<var>name of the registry</var>
</pre>")
    }

    async fn list_owners(&self, token: &str, _crate_name: &str) -> AuthResult<Vec<ListedOwner>> {
        self.validated_user_id(token).await?;

        Ok(vec![ListedOwner {
            id: 0,
            login: self.team_base_url.clone(),
            name: None,
        }])
    }

    async fn add_owners(&self, token: &str, _users: &[&str], _crate_name: &str) -> AuthResult<()> {
        // everyone is an owner, so it's technically a no-op
        self.validated_non_service_user_id(token).await?;
        Ok(())
    }

    async fn remove_owners(
        &self,
        token: &str,
        _users: &[&str],
        _crate_name: &str,
    ) -> AuthResult<()> {
        self.validated_non_service_user_id(token).await?;
        Err(AuthError::Unimplemented)
    }

    async fn publish(&self, token: &str, _crate_name: &str) -> AuthResult<()> {
        // only CI (using service token) is allowed to publish
        let id = self.validated_user_id(token).await?;
        self.authorize_publish(&id)
    }

    async fn auth_yank(&self, token: &str, _crate_name: &str) -> AuthResult<()> {
        self.validated_non_service_user_id(token).await?;
        Ok(())
    }

    /// Fetch of config.json.
    async fn auth_config(&self, token: &str) -> AuthResult<()> {
        self.validated_user_id(token).await?;
        Ok(())
    }

    async fn auth_index_fetch(
        &self,
        token: &str,
        _all_users_can_read_crates: &str,
    ) -> AuthResult<()> {
        self.validated_user_id(token).await?;
        Ok(())
    }

    async fn auth_crate_download(
        &self,
        token: &str,
        _all_users_can_read_crates: &str,
    ) -> AuthResult<()> {
        self.validated_user_id(token).await?;
        Ok(())
    }

    async fn auth_view_full_index(&self, token: &str) -> AuthResult<()> {
        self.validated_user_id(token).await?;
        Ok(())
    }

    fn token_from_headers<'h>(
        &self,
        headers: &'h HeaderMap,
    ) -> Result<Option<&'h str>, StatusCode> {
        if let Some(res) = crate::default_token_from_headers(headers)? {
            return Ok(Some(res.strip_prefix("CF_Authorization=").unwrap_or(res)));
        }
        if let Some(cookies) = headers.get(header::COOKIE) {
            let cookies = cookies.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;
            for c in cookie::Cookie::split_parse(cookies) {
                let c = c.map_err(|_| StatusCode::BAD_REQUEST)?;
                if c.name() == "CF_Authorization" {
                    return Ok(c.value_raw());
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(publish: &[&str]) -> Config {
        Config {
            auth_audience: "...".into(),
            auth_team_base_url: "https://test.example.net".into(),
            auth_publish_access_ids: publish.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    #[test]
    fn cookie_parse() {
        let a = CfAuthProvider::new(config(&[])).unwrap();

        let mut h = http::HeaderMap::new();
        h.insert("cookie", http::HeaderValue::from_static("other.cookie=1; lastViewedForm-TEST={}; JSESSIONID=EE; CF_AppSession=2; CF_Authorization=aaaaaaaaa.bbbbbbb.cccccc; X=1"));

        let cookie = a.token_from_headers(&h).unwrap().unwrap();
        assert_eq!("aaaaaaaaa.bbbbbbb.cccccc", cookie);
    }

    #[test]
    fn service_identities_cannot_authorize_non_publish_mutations() {
        let auth = CfAuthProvider::new(config(&["publisher.access"])).unwrap();

        assert!(matches!(
            CfAuthProvider::authorize_non_service_mutation(&UserId("reader.access".into())),
            Err(AuthError::Forbidden)
        ));
        assert!(matches!(
            CfAuthProvider::authorize_non_service_mutation(&UserId("publisher.access".into())),
            Err(AuthError::Forbidden)
        ));
        assert!(
            CfAuthProvider::authorize_non_service_mutation(&UserId("user@example.com".into()))
                .is_ok()
        );

        // The publisher allowlist grants only publish permission.
        assert!(
            auth.authorize_publish(&UserId("publisher.access".into()))
                .is_ok()
        );
    }

    #[test]
    fn publish_permission_requires_allowlisted_service_identity() {
        let auth = CfAuthProvider::new(config(&["publisher.access"])).unwrap();

        assert!(
            auth.authorize_publish(&UserId("publisher.access".into()))
                .is_ok()
        );
        assert!(matches!(
            auth.authorize_publish(&UserId("reader.access".into())),
            Err(AuthError::Forbidden)
        ));
        assert!(matches!(
            auth.authorize_publish(&UserId("user@example.com".into())),
            Err(AuthError::Forbidden)
        ));
    }
}
