#![cfg_attr(docsrs, feature(doc_cfg))]

use async_trait::async_trait;

#[cfg(feature = "yes-backend")]
#[cfg_attr(docsrs, doc(cfg(feature = "yes-backend")))]
pub mod yes_backend;

pub mod no_backend;

#[cfg(feature = "pg-backend")]
#[cfg_attr(docsrs, doc(cfg(feature = "pg-backend")))]
pub mod pg_backend;

#[cfg(feature = "fs-backend")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs-backend")))]
pub mod fs_backend;

#[cfg(feature = "fs-backend")]
mod base64_serde;

mod error;

pub use error::*;
use freighter_api_types::ownership::response::ListedOwner;

#[async_trait]
pub trait AuthProvider {
    type Config;

    /// Register a new user, returning a token if successful.
    async fn register(&self, username: &str) -> AuthResult<String>;

    /// List the owners of a crate.
    async fn list_owners(&self, token: &str, crate_name: &str) -> AuthResult<Vec<ListedOwner>>;
    /// Add a new owner to a crate.
    async fn add_owners(&self, token: &str, users: &[&str], crate_name: &str) -> AuthResult<()>;
    /// Remove an owner from a crate.
    async fn remove_owners(&self, token: &str, users: &[&str], crate_name: &str) -> AuthResult<()>;

    /// Verify that a user has permission to publish new versions of a crate.
    ///
    /// If the crate has never been published before to the registry, the user should be given
    /// ownership of the new crate.
    async fn publish(&self, token: &str, crate_name: &str) -> AuthResult<()>;

    /// Verify that a user has permission to yank or unyank versions of a crate.
    async fn auth_yank(&self, token: &str, crate_name: &str) -> AuthResult<()>;

    /// Verify that a user is allowed to look at the index entry for a given crate.
    ///
    /// A default implementation is provided which allows access categorically.
    /// This is currently only meaningful for registries which rely on experimental cargo features
    /// to auth any access to the registry.
    async fn auth_index_fetch(&self, token: Option<&str>, crate_name: &str) -> AuthResult<()> {
        let _ = (token, crate_name);
        Ok(())
    }

    /// Verify that a user is allowed to download a given crate.
    ///
    /// A default implementation is provided which allows access categorically.
    /// This is currently only meaningful for registries which rely on experimental cargo features
    /// to auth any access to the registry.
    async fn auth_crate_download(&self, token: Option<&str>, crate_name: &str) -> AuthResult<()> {
        let _ = (token, crate_name);
        Ok(())
    }

    /// Verify that a user is allowed to view the full index.
    ///
    /// This is used for both searching the index and listing all crates.
    ///
    /// A default implementation is provided which allows access.
    /// This is currently only meaningful for registries which rely on experimental cargo features
    /// to auth any access to the registry.
    async fn auth_view_full_index(&self, token: Option<&str>) -> AuthResult<()> {
        let _ = token;
        Ok(())
    }
}
