#![allow(clippy::type_complexity)]
use anyhow::Context;
use async_trait::async_trait;
use chrono::Utc;
use freighter_api_types::index::request::{ListQuery, Publish};
use freighter_api_types::index::response::{
    CompletedPublication, CrateVersion, Dependency, ListAll, ListAllCrateEntry,
    ListAllCrateVersion, SearchResults, SearchResultsEntry, SearchResultsMeta,
};
use freighter_api_types::index::{IndexError, IndexProvider, IndexResult, SparseEntries};
use freighter_api_types::storage::MetadataStorageProvider;
use freighter_storage::fs::FsStorageProvider;
use freighter_storage::s3_client::S3StorageProvider;
use semver::Version;
use serde::Deserialize;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::task::JoinSet;

mod file_locks;
use file_locks::{AccessLocks, CrateMetaPath};

use crate::file_locks::deserialize_data;

pub struct FsIndexProvider {
    meta_file_locks: AccessLocks<String>,
    fs: Arc<dyn MetadataStorageProvider + Send + Sync>,
}

impl FsIndexProvider {
    pub fn new(config: Config) -> IndexResult<Self> {
        let fs = match config {
            Config::Path(root) => Arc::new(FsStorageProvider::new(root)?)
                as Arc<dyn MetadataStorageProvider + Send + Sync>,
            Config::S3(c) => Arc::new(S3StorageProvider::new(
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

        let (mut releases, publish) = meta.deserialized().await?;
        let release = releases.entries
            .iter_mut()
            .rfind(|v| &v.vers == version)
            .ok_or(IndexError::NotFound)?;
        release.yanked = yank;
        meta.replace(&releases.entries, publish.as_ref()).await
    }

    const fn is_valid_crate_file_name_char(c: u8) -> bool {
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

        let len = lc_crate_name.len().checked_add(12)?;
        let mut path = String::with_capacity(len);

        path.push_str("index/");

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

    async fn get_sparse_entry(&self, crate_name: &str) -> IndexResult<SparseEntries> {
        self.access_crate(crate_name)?
            .shared()
            .await
            .deserialized()
            .await
            .map(|(versions, _)| versions)
    }

    async fn confirm_existence(&self, crate_name: &str, version: &Version) -> IndexResult<bool> {
        self.access_crate(crate_name)?
            .shared()
            .await
            .deserialized()
            .await
            .map(|(versions, _)| versions.entries)?
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
        publish: &Publish,
        checksum: &str,
        end_step: Pin<&mut (dyn Future<Output = IndexResult<()>> + Send)>,
    ) -> IndexResult<CompletedPublication> {
        let release = CrateVersion {
            name: publish.name.clone(),
            vers: publish.vers.clone(),
            deps: publish
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
            features: publish.features.clone(),
            yanked: false,
            links: publish.links.clone(),
            v: 2,
            features2: HashMap::default(),
        };

        let lock = self.access_crate(&release.name)?;
        let meta = lock.exclusive().await;

        let mut versions = match meta.deserialized().await {
            Ok((existing_releases, _)) => {
                if existing_releases.entries.iter().any(|v| v.vers == release.vers) {
                    return Err(IndexError::Conflict(format!(
                        "{}-{} aleady exists",
                        publish.name, publish.vers
                    )));
                }

                existing_releases.entries
            }
            Err(IndexError::NotFound) => vec![],
            Err(other) => return Err(other),
        };
        versions.push(release);

        end_step.await?;

        meta.put_index_file(&versions, publish).await?;
        Ok(CompletedPublication { warnings: None })
    }

    async fn list(&self, _pagination: &ListQuery) -> IndexResult<ListAll> {
        let index_keys = self.fs.list_prefix("index/").await?;
        let mut results = Vec::with_capacity(index_keys.len());

        let mut crate_versions_with_publish =
            get_latest_crate_publishes(Arc::clone(&self.fs), index_keys);

        while let Some(handle) = crate_versions_with_publish.join_next().await {
            let publish_fetch_result = handle.context("index fetch task unexpectedly failed")?;

            match publish_fetch_result {
                Ok((versions, publish)) => {
                    let entry = convert_publish_to_crate_entry(versions, publish);
                    results.push(entry);
                }
                Err(error) => {
                    tracing::error!(%error, "failed to get Publish information for crate");
                }
            }
        }

        Ok(ListAll { results })
    }

    async fn search(&self, query_string: &str, limit: usize) -> IndexResult<SearchResults> {
        let mut index_keys = self.fs.list_prefix("index/").await?;
        index_keys.retain(|k| k.contains(query_string));
        let total = index_keys.len();
        index_keys.truncate(limit.min(100));
        let mut results = Vec::with_capacity(index_keys.len());

        let mut crate_versions_with_publish =
            get_latest_crate_publishes(Arc::clone(&self.fs), index_keys);

        while let Some(handle) = crate_versions_with_publish.join_next().await {
            let Ok(Ok((mut versions, publish_meta))) = handle else { continue };
            let Some(most_recent_version) = versions.entries.pop() else { continue };
            results.push(SearchResultsEntry {
                name: most_recent_version.name,
                max_version: most_recent_version.vers,
                description: publish_meta.and_then(|p| p.description).unwrap_or_default(),
            });
        }

        Ok(SearchResults {
            crates: results,
            meta: SearchResultsMeta { total },
        })
    }
}

fn get_latest_crate_publishes(
    fs: Arc<dyn MetadataStorageProvider + Send + Sync>,
    index_keys: Vec<String>,
) -> JoinSet<IndexResult<(Vec<CrateVersion>, Option<Publish>)>> {
    let mut join_set = JoinSet::new();

    for index_key in index_keys {
        let fs = Arc::clone(&fs);
        join_set.spawn(async move {
            let (versions, publish) = fs
                .pull_file(&index_key)
                .await
                .map(|res| deserialize_data(&res.data))?
                .context("deserializing index file for list")?;

            Ok((versions, publish))
        });
    }

    join_set
}

fn convert_publish_to_crate_entry(
    mut versions: Vec<CrateVersion>,
    publish: Option<Publish>,
) -> ListAllCrateEntry {
    versions.sort_by_key(|v| Reverse(v.vers.clone()));

    let publish = publish.unwrap_or_else(|| Publish {
        name: versions[0].name.to_string(),
        vers: versions[0].vers.clone(),
        ..Publish::empty()
    });

    ListAllCrateEntry {
        name: publish.name.clone(),
        versions: versions
            .into_iter()
            .map(|version| ListAllCrateVersion {
                version: version.vers,
            })
            .collect(),
        description: publish.description.unwrap_or_default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        homepage: publish.homepage.clone(),
        repository: publish.repository.clone(),
        documentation: publish.documentation.clone(),
        keywords: publish.keywords.clone(),
        categories: publish.categories,
    }
}
