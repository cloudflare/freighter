//! A backend that says "yes" to every request for authorization.
//!
//! This is exactly as insecure as it sounds, and is meant primarily for testing purposes.

use crate::{AuthProvider, AuthResult};
use async_trait::async_trait;
use freighter_api_types::ownership::response::ListedOwner;
use rand::distributions::{Alphanumeric, DistString};

pub struct YesAuthProvider;

impl YesAuthProvider {
    pub fn new(_yes_config: ()) -> AuthResult<Self> {
        Ok(YesAuthProvider)
    }
}

#[async_trait]
impl AuthProvider for YesAuthProvider {
    type Config = ();

    async fn healthcheck(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn register(&self, _username: &str) -> AuthResult<String> {
        let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

        Ok(token)
    }

    async fn list_owners(&self, _token: &str, _crate_name: &str) -> AuthResult<Vec<ListedOwner>> {
        Ok(Vec::new())
    }

    async fn add_owners(&self, _token: &str, _users: &[&str], _crate_name: &str) -> AuthResult<()> {
        Ok(())
    }

    async fn remove_owners(
        &self,
        _token: &str,
        _users: &[&str],
        _crate_name: &str,
    ) -> AuthResult<()> {
        Ok(())
    }

    async fn publish(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        Ok(())
    }

    async fn auth_yank(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        Ok(())
    }

    async fn auth_config(&self, _token: &str) -> AuthResult<()> {
        Ok(())
    }

    async fn auth_index_fetch(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        Ok(())
    }

    async fn auth_crate_download(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        Ok(())
    }

    async fn auth_view_full_index(&self, _token: &str) -> AuthResult<()> {
        Ok(())
    }
}
