//! Useless (but safe) placeholder for a backend
use crate::{AuthProvider, AuthError, AuthResult};
use async_trait::async_trait;
use freighter_api_types::ownership::response::ListedOwner;

pub struct NoAuthProvider;

fn nope<T>() -> AuthResult<T> {
    Err(AuthError::Unimplemented)
}

impl NoAuthProvider {
    pub fn new(_no_config: ()) -> AuthResult<Self> {
        nope()
    }
}

/// Used as fallback to avoid compile error when no backend is configured
#[async_trait]
impl AuthProvider for NoAuthProvider {
    type Config = ();

    async fn register(&self, _username: &str) -> AuthResult<String> {
        nope()
    }

    async fn list_owners(&self, _token: &str, _crate_name: &str) -> AuthResult<Vec<ListedOwner>> {
        nope()
    }

    async fn add_owners(&self, _token: &str, _users: &[&str], _crate_name: &str) -> AuthResult<()> {
        nope()
    }

    async fn remove_owners(&self, _token: &str, _users: &[&str], _crate_name: &str) -> AuthResult<()> {
        nope()
    }

    async fn publish(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        nope()
    }

    async fn auth_yank(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        nope()
    }

    async fn auth_index_fetch(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        nope()
    }

    async fn auth_crate_download(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        nope()
    }

    async fn auth_view_full_index(&self, _token: &str) -> AuthResult<()> {
        nope()
    }
}
