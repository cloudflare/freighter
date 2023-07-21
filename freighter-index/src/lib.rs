use async_trait::async_trait;
use semver::Version;

use std::future::Future;
use std::pin::Pin;

#[cfg(feature = "postgresql-backend")]
pub mod postgres_client;

mod api_types;

mod error;

pub type IndexResult<T> = Result<T, IndexError>;

pub use api_types::*;
pub use error::*;

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
    /// Get the sparse index entry for a crate.
    ///
    /// If successful, a [`CrateVersion`] api object will be returned.
    ///
    /// If the crate could not be found in the index, [`None`] will be returned.
    ///
    /// If an error occurs while trying to generate the sparse entry, [`IndexError::ServiceError`]
    /// will be returned.
    async fn get_sparse_entry(&self, crate_name: &str) -> IndexResult<Vec<CrateVersion>>;
    /// Confirm that a particular crate and version pair exists, and return its yank status
    async fn confirm_existence(&self, crate_name: &str, version: &Version) -> IndexResult<bool>;
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
        checksum: &str,
        end_step: Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>,
    ) -> IndexResult<CompletedPublication>;
    /// List crates in the index, optionally specifying pagination.
    ///
    /// If no pagination is provided, all crates should be returned.
    async fn list(&self, pagination: &ListQuery) -> IndexResult<ListAll>;
}
