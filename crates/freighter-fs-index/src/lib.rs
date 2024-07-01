use async_trait::async_trait;
use freighter_api_types::index::request::{ListQuery, Publish};
use freighter_api_types::index::response::{
    CompletedPublication, CrateVersion, Dependency, ListAll, SearchResults,
};
use freighter_api_types::index::{IndexError, IndexProvider, IndexResult};
use freighter_api_types::storage::MetadataStorageProvider;
use freighter_storage::fs::FsStorageProvider;
use freighter_storage::s3_client::S3StorageProvider;
use semver::Version;
use serde::Deserialize;
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;

mod file_locks;
use file_locks::{AccessLocks, CrateMetaPath};

pub struct FsIndexProvider {
    meta_file_locks: AccessLocks<String>,
    fs: Box<dyn MetadataStorageProvider + Send + Sync>,
}

impl FsIndexProvider {
    pub fn new(config: Config) -> IndexResult<Self> {
        let fs = match config {
            Config::Path(root) => Box::new(FsStorageProvider::new(root)?)
                as Box<dyn MetadataStorageProvider + Send + Sync>,
            Config::S3(c) => Box::new(S3StorageProvider::new(
                &c.name,
                &c.endpoint_url,
                &c.region,
                &c.access_key_id.unwrap_or_else(|| {
                    std::env::var("FREIGHTER_INDEX_BUCKET_KEY_ID").expect(
                        "Failed to find index bucket key id in environment variable or config",
                    )
                }),
                &c.access_key_secret.unwrap_or_else(|| {
                    std::env::var("FREIGHTER_INDEX_BUCKET_KEY_SECRET").expect(
                        "Failed to find index bucket key secret in environment variable or config",
                    )
                }),
            )),
        };
        Ok(Self {
            fs,
            meta_file_locks: AccessLocks::new(),
        })
    }

    pub(crate) fn access_crate(&self, crate_name: &str) -> IndexResult<CrateMetaPath<'_>> {
        let lowercase_name = crate_name.to_ascii_lowercase();
        let meta_file_rel_path = self
            .crate_meta_file_rel_path(&lowercase_name)
            .ok_or(IndexError::CrateNameNotAllowed)?;
        Ok(CrateMetaPath::new(
            &*self.fs,
            &self.meta_file_locks,
            lowercase_name,
            meta_file_rel_path,
        ))
    }

    async fn yank_inner(&self, crate_name: &str, version: &Version, yank: bool) -> IndexResult<()> {
        let lock = self.access_crate(crate_name)?;
        let meta = lock.exclusive().await;

        let mut releases = meta.deserialized().await?;
        let release = releases
            .iter_mut()
            .rfind(|v| &v.vers == version)
            .ok_or(IndexError::NotFound)?;
        release.yanked = yank;
        meta.replace(&releases).await
    }

    fn is_valid_crate_file_name_char(c: u8) -> bool {
        (c.is_ascii_alphabetic() && c.is_ascii_lowercase())
            || c.is_ascii_digit()
            || c == b'-'
            || c == b'_'
    }

    /// Crate name must be already lowercased
    #[allow(clippy::unused_self)]
    fn crate_meta_file_rel_path(&self, lc_crate_name: &str) -> Option<String> {
        if lc_crate_name.len() > 64
            || !lc_crate_name
                .bytes()
                .all(Self::is_valid_crate_file_name_char)
        {
            return None;
        }

        let len = lc_crate_name.len().checked_add(5)?;
        let mut path = String::with_capacity(len);

        match lc_crate_name.len() {
            4.. => {
                let (prefix1, prefix2) = lc_crate_name[..4].split_at(2);
                path.push_str(prefix1);
                path.push('/');
                path.push_str(prefix2);
            }
            1 => path.push('1'),
            2 => path.push('2'),
            3 => {
                path.push_str("3/");
                path.push_str(&lc_crate_name[..1]);
            }
            _ => return None,
        };
        path.push('/');
        path.push_str(lc_crate_name);
        Some(path)
    }
}

#[derive(Deserialize)]
pub struct StoreConfig {
    pub name: String,
    pub endpoint_url: String,
    pub region: String,
    pub access_key_id: Option<String>,
    pub access_key_secret: Option<String>,
}

#[derive(Deserialize)]
pub enum Config {
    #[serde(rename = "index_path")]
    Path(PathBuf),
    #[serde(rename = "index_s3")]
    S3(StoreConfig),
}

#[async_trait]
impl IndexProvider for FsIndexProvider {
    type Config = Config;

    async fn healthcheck(&self) -> anyhow::Result<()> {
        self.fs.healthcheck().await?;
        Ok(())
    }

    async fn get_sparse_entry(&self, crate_name: &str) -> IndexResult<Vec<CrateVersion>> {
        self.access_crate(crate_name)?
            .shared()
            .await
            .deserialized()
            .await
    }

    async fn confirm_existence(&self, crate_name: &str, version: &Version) -> IndexResult<bool> {
        self.access_crate(crate_name)?
            .shared()
            .await
            .deserialized()
            .await?
            .iter()
            .rfind(|e| &e.vers == version)
            .map(|e| e.yanked)
            .ok_or(IndexError::NotFound)
    }

    async fn yank_crate(&self, crate_name: &str, version: &Version) -> IndexResult<()> {
        self.yank_inner(crate_name, version, true).await
    }

    async fn unyank_crate(&self, crate_name: &str, version: &Version) -> IndexResult<()> {
        self.yank_inner(crate_name, version, false).await
    }

    async fn publish(
        &self,
        p: &Publish,
        checksum: &str,
        end_step: Pin<&mut (dyn Future<Output = IndexResult<()>> + Send)>,
    ) -> IndexResult<CompletedPublication> {
        let release = CrateVersion {
            name: p.name.clone(),
            vers: p.vers.clone(),
            deps: p
                .deps
                .iter()
                .map(|d| {
                    let (alias, package) = if let Some(renamed) = &d.explicit_name_in_toml {
                        (renamed.clone(), Some(d.name.clone()))
                    } else {
                        (d.name.clone(), None)
                    };
                    Dependency {
                        name: alias,
                        req: d.version_req.clone(),
                        features: d.features.clone(),
                        optional: d.optional,
                        default_features: d.default_features,
                        target: d.target.clone(),
                        kind: d.kind,
                        registry: d.registry.clone(),
                        package,
                    }
                })
                .collect(),
            cksum: checksum.into(),
            features: p.features.clone(),
            yanked: false,
            links: p.links.clone(),
            v: 2,
            features2: HashMap::default(),
        };

        let lock = self.access_crate(&release.name)?;
        let meta = lock.exclusive().await;

        match meta.deserialized().await {
            Ok(existing_releases) => {
                if existing_releases.iter().any(|v| v.vers == release.vers) {
                    return Err(IndexError::Conflict(format!(
                        "{}-{} aleady exists",
                        p.name, p.vers
                    )));
                }
            }
            Err(IndexError::NotFound) => {}
            Err(other) => return Err(other),
        };

        end_step.await?;
        meta.create_or_append(&release).await?;
        Ok(CompletedPublication { warnings: None })
    }

    async fn list(&self, _pagination: &ListQuery) -> IndexResult<ListAll> {
        Err(IndexError::ServiceError(
            io::Error::from(io::ErrorKind::Unsupported).into(),
        ))
    }

    async fn search(&self, _query_string: &str, _limit: usize) -> IndexResult<SearchResults> {
        Err(IndexError::ServiceError(
            io::Error::from(io::ErrorKind::Unsupported).into(),
        ))
    }
}
