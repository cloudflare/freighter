use async_trait::async_trait;

#[cfg(feature = "yes-backend")]
pub mod yes_backend;

#[cfg(feature = "pg-backend")]
pub mod pg_backend;

mod api_types;
mod error;

pub use api_types::*;
pub use error::*;

#[async_trait]
pub trait AuthClient {
    /// Register a new user, returning a token if successful.
    async fn register(&self, username: &str, password: &str) -> AuthResult<String>;
    /// Retrieve a token for an existing user if the credentials match.
    async fn login(&self, username: &str, password: &str) -> AuthResult<String>;

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
    /// Verify that a user has permission to yank versions of a crate.
    async fn auth_yank(&self, token: &str, crate_name: &str) -> AuthResult<()>;
    /// Verify that a user has permission to unyank versions of a crate.
    async fn auth_unyank(&self, token: &str, crate_name: &str) -> AuthResult<()>;
}
