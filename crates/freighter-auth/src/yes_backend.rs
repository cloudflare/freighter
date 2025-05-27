//! A backend that says "yes" to every request for authorization.
//!
//! This is exactly as insecure as it sounds, and is meant primarily for testing purposes.

use crate::{AuthProvider, AuthResult};
use async_trait::async_trait;
use freighter_api_types::ownership::response::ListedOwner;
use rand::distr::{Alphanumeric, SampleString};

/// In the config specify `auth_allow_full_access_without_any_checks: true` to give full access to the registry,
/// including crate publishing, to anyone who can connect to it.
pub struct YesAuthProvider(());

impl YesAuthProvider {
    #[track_caller]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(yes_config: Config) -> AuthResult<Self> {
        if !yes_config.auth_allow_full_access_without_any_checks {
            return Err(anyhow::anyhow!("enabled 'yes' auth without explicit opt-in").into());
        }
        Ok(Self(()))
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct Config {
    pub auth_allow_full_access_without_any_checks: bool,
}

#[async_trait]
impl AuthProvider for YesAuthProvider {
    type Config = Config;

    async fn healthcheck(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn register(&self, _username: &str) -> AuthResult<String> {
        let token = Alphanumeric.sample_string(&mut rand::rng(), 32);

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
