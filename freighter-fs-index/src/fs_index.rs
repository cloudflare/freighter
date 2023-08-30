use anyhow::Context;
use async_trait::async_trait;
use freighter_api_types::index::request::{ListQuery, Publish};
use freighter_api_types::index::response::{
    CompletedPublication, CrateVersion, Dependency, ListAll, SearchResults, ListAllCrateEntry, ListAllCrateVersion,
};
use freighter_api_types::index::{IndexError, IndexProvider, IndexResult};
use semver::Version;
use serde::Deserialize;
use std::ffi::OsStr;
use std::future::Future;
use std::{io, debug_assert_eq};
use std::os::unix::ffi::OsStrExt;
use std::path::{PathBuf, Path};
use std::pin::Pin;
use std::time::SystemTime;

mod file_locks;
use file_locks::{CrateMetaPath, FileLocks};

pub struct FsIndexProvider {
    meta_file_locks: FileLocks,
    root: PathBuf,
}

impl FsIndexProvider {
    pub fn new(config: Config) -> IndexResult<Self> {
        let root = config.index_path;
        std::fs::create_dir_all(&root)
            .with_context(|| format!("Index root at {}", root.display()))
            .map_err(IndexError::ServiceError)?;
        Ok(Self {
            root,
            meta_file_locks: Default::default(),
        })
    }

    fn access_crate_at_path(&self, path: PathBuf) -> IndexResult<CrateMetaPath<'_>> {
        let name = path.file_name().and_then(|f| f.to_str())
            .ok_or(IndexError::CrateNameNotAllowed)?
            .to_string();
        debug_assert_eq!(name, name.to_ascii_lowercase());
        Ok(CrateMetaPath::new(path, name, &self.meta_file_locks))
    }

    pub(crate) fn access_crate(&self, crate_name: &str) -> IndexResult<CrateMetaPath<'_>> {
        let lowercase_name = crate_name.to_ascii_lowercase();
        let meta_file_path = self.crate_meta_file_path(&lowercase_name).ok_or(IndexError::CrateNameNotAllowed)?;
        Ok(CrateMetaPath::new(meta_file_path, lowercase_name, &self.meta_file_locks))
    }

    async fn yank_inner(&self, crate_name: &str, version: &Version, yank: bool) -> IndexResult<()> {
        let lock = self.access_crate(crate_name)?;
        let meta = lock.exclusive().await;

        let mut releases = meta.deserialized()?;
        let release = releases.iter_mut()
            .rfind(|v| &v.vers == version)
            .ok_or(IndexError::NotFound)?;
        release.yanked = yank;
        meta.replace(&releases)
    }

    fn is_valid_crate_file_name_char(c: u8) -> bool {
        (c.is_ascii_alphabetic() && c.is_ascii_lowercase()) ||
        c.is_ascii_digit() ||
        c == b'-' || c == b'_'
    }

    /// Crate name must be already lowercased
    fn crate_meta_file_path(&self, lc_crate_name: &str) -> Option<PathBuf> {
        if lc_crate_name.len() > 64 || !lc_crate_name.bytes().all(Self::is_valid_crate_file_name_char) {
            return None;
        }

        let len = self.root.as_os_str().len().checked_add(lc_crate_name.len())?.checked_add(5)?;
        let mut path = PathBuf::with_capacity(len);
        path.push(&self.root);

        match lc_crate_name.len() {
            4.. => {
                let (prefix1, prefix2) = lc_crate_name.as_bytes()[..4].split_at(2);
                path.push(OsStr::from_bytes(prefix1));
                path.push(OsStr::from_bytes(prefix2));
            },
            1 => path.push("1"),
            2 => path.push("2"),
            3 => {
                path.push("3");
                path.push(OsStr::from_bytes(&lc_crate_name.as_bytes()[..1]));
            },
            _ => return None,
        };
        path.push(lc_crate_name);
        Some(path)
    }

    /// Assumes all files are crates
    fn list_crates_in_subdir<'a, 'b: 'a>(&'b self, path: &Path, out: &'a mut Vec<ListAllCrateEntry>) -> Pin<Box<dyn Future<Output=IndexResult<()>> + Send + Sync + 'a>> {
        let dir = std::fs::read_dir(path);
        Box::pin(async move {
            for entry in dir.map_err(|e| IndexError::ServiceError(e.into()))? {
                let Ok(entry) = entry else { continue };
                let Ok(metadata) = entry.metadata() else { continue };
                let path = entry.path();
                if metadata.is_dir() {
                    self.list_crates_in_subdir(&path, out).await?;
                } else if metadata.is_file() {
                    let mut releases = self.access_crate_at_path(path)?.shared().await.deserialized()?;
                    let mut versions = Vec::with_capacity(releases.len());
                    let Some(most_recent) = releases.pop() else { continue };
                    versions.extend(releases.into_iter().map(|r| ListAllCrateVersion { version: r.vers })
                        .chain(Some(ListAllCrateVersion { version: most_recent.vers })));
                    let created_at = metadata.created().unwrap_or_else(|_| SystemTime::now());
                    let updated_at = metadata.modified().unwrap_or(created_at);
                    out.push(ListAllCrateEntry {
                        versions,
                        name: most_recent.name,
                        description: String::new(),
                        created_at: created_at.into(),
                        updated_at: updated_at.into(),
                        homepage: None,
                        repository: None,
                        documentation: None,
                        keywords: Vec::new(),
                        categories: Vec::new(),
                    });
                }
            }
            Ok(())
        })
    }
}

#[derive(Deserialize)]
pub struct Config {
    pub index_path: PathBuf,
}

#[async_trait]
impl IndexProvider for FsIndexProvider {
    type Config = Config;

    async fn get_sparse_entry(&self, crate_name: &str) -> IndexResult<Vec<CrateVersion>> {
        self.access_crate(crate_name)?.shared().await.deserialized()
    }

    async fn confirm_existence(&self, crate_name: &str, version: &Version) -> IndexResult<bool> {
        self.access_crate(crate_name)?.shared().await
            .deserialized()?
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
        end_step: Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>,
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
            features2: Default::default(),
        };

        let lock = self.access_crate(&release.name)?;
        let meta = lock.exclusive().await;

        match meta.deserialized() {
            Ok(existing_releases) => {
                if existing_releases.iter().any(|v| v.vers == release.vers) {
                    return Err(IndexError::Conflict(format!("{}-{} aleady exists", p.name, p.vers)));
                }
            },
            Err(IndexError::NotFound) => {},
            Err(other) => return Err(other),
        };

        end_step.await?;
        meta.create_or_append(&release)?;
        Ok(CompletedPublication { warnings: None })
    }

    async fn list(&self, _pagination: &ListQuery) -> IndexResult<ListAll> {
        let mut results = Vec::new();

        // top level of the index can have config and other stuff
        for entry in std::fs::read_dir(&self.root).map_err(|e| IndexError::ServiceError(e.into()))? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|f| f.to_str()) else { continue };
            if file_name.len() > 2 || file_name.starts_with('.') { continue; }

            self.list_crates_in_subdir(&path, &mut results).await?;
        }

        Ok(ListAll {
            results,
        })
    }

    async fn search(&self, _query_string: &str, _limit: usize) -> IndexResult<SearchResults> {
        Err(IndexError::ServiceError(io::Error::from(io::ErrorKind::Unsupported).into()))
    }
}
