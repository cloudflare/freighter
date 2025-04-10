use async_trait::async_trait;
use chrono::{DateTime, Utc};
use semver::Version;
use std::future::Future;
use std::pin::Pin;

use request::{ListQuery, Publish, PublishDependency};
use response::{CompletedPublication, CrateVersion, ListAll, SearchResults};

#[cfg(any(feature = "index", feature = "server", feature = "client"))]
use serde::{Deserialize, Serialize};

pub mod request;
pub mod response;

mod error;
pub use error::*;

pub type IndexResult<T> = Result<T, IndexError>;

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(
    any(feature = "index", feature = "server", feature = "client"),
    derive(Serialize, Deserialize),
    serde(rename_all = "lowercase")
)]
#[cfg_attr(
    feature = "postgres",
    derive(postgres_types::ToSql, postgres_types::FromSql),
    postgres(name = "dependency_kind")
)]
pub enum DependencyKind {
    #[cfg_attr(feature = "postgres", postgres(name = "normal"))]
    #[default]
    Normal,
    #[cfg_attr(feature = "postgres", postgres(name = "dev"))]
    Dev,
    #[cfg_attr(feature = "postgres", postgres(name = "build"))]
    Build,
}

impl From<response::CrateVersion> for request::Publish {
    fn from(value: CrateVersion) -> Self {
        Self {
            name: value.name,
            vers: value.vers,
            deps: value
                .deps
                .into_iter()
                .map(|x| {
                    // This is the opposite of how Cargo.toml does it
                    let (package_name, explicit_name_in_toml) = if let Some(package) = x.package {
                        (package, Some(x.name))
                    } else {
                        (x.name, None)
                    };
                    PublishDependency {
                        name: package_name,
                        version_req: x.req,
                        features: x.features,
                        optional: x.optional,
                        default_features: x.default_features,
                        target: x.target,
                        kind: x.kind,
                        registry: x.registry,
                        explicit_name_in_toml,
                    }
                })
                .collect(),
            features: value.features,
            // Note: We do not carry over authors since its not in index
            authors: Vec::new(),
            description: None,
            documentation: None,
            homepage: None,
            readme: None,
            readme_file: None,
            keywords: vec![],
            categories: vec![],
            license: None,
            license_file: None,
            repository: None,
            badges: None,
            links: None,
        }
    }
}

pub struct SparseEntries {
    pub entries: Vec<CrateVersion>,
    pub last_modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy)]
pub struct CrateVersionExists {
    pub yanked: bool,
    /// Sha256 of the package
    pub tarball_checksum: [u8; 32],
}

/// A client for talking with a backing index database or storage medium.
///
/// Operations performed via this client MUST be atomic.
/// In the event of a conflict, [`IndexError::Conflict`] should be returned by an operation.
///
/// # Note
/// The index client does NOT authenticate user actions.
/// User actions should be authenticated before an operation is performed.
#[async_trait]
pub trait IndexProvider: Sync {
    type Config
    where
        Self: Sized;

    async fn healthcheck(&self) -> anyhow::Result<()>;

    /// Get the sparse index entry for a crate.
    ///
    /// If successful, a [`CrateVersion`] api object will be returned.
    ///
    /// If the crate could not be found in the index, [`None`] will be returned.
    ///
    /// If an error occurs while trying to generate the sparse entry, [`IndexError::ServiceError`]
    /// will be returned.
    async fn get_sparse_entry(&self, crate_name: &str) -> IndexResult<SparseEntries>;
    /// Confirm that a particular crate and version pair exists, and return its yank status
    async fn confirm_existence(
        &self,
        crate_name: &str,
        version: &Version,
    ) -> IndexResult<CrateVersionExists>;
    /// Yank a crate version.
    async fn yank_crate(&self, crate_name: &str, version: &Version) -> IndexResult<()>;
    /// Unyank a crate version
    async fn unyank_crate(&self, crate_name: &str, version: &Version) -> IndexResult<()>;
    /// Search the index for crates satisfying a query string, returning up to `limit` results.
    ///
    /// The syntax and semantics of the search are up to the implementation to define.
    async fn search(&self, query_string: &str, limit: usize) -> IndexResult<SearchResults>;
    /// Publish a crate version.
    ///
    /// `end_step` is a future to run after the crate has been submitted to the index, but before
    /// any transactional commits have occurred.
    /// If it fails, the operation MUST be rolled back.
    async fn publish(
        &self,
        version: &Publish,
        tarball_checksum: [u8; 32],
        end_step: Pin<&mut (dyn Future<Output = IndexResult<()>> + Send)>,
    ) -> IndexResult<CompletedPublication>;
    /// List crates in the index, optionally specifying pagination.
    ///
    /// If no pagination is provided, all crates should be returned.
    async fn list(&self, pagination: &ListQuery) -> IndexResult<ListAll>;
}
