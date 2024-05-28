use crate::cf_access::{CfAccess, UserId};
use crate::{AuthError, AuthProvider, AuthResult};
use async_trait::async_trait;
use freighter_api_types::ownership::response::ListedOwner;
use serde::Deserialize;

/// Registry auth based on Cloudflare Access, using JsonWebTokens for auth
pub struct CfAuthProvider {
    team_base_url: String,
    access: CfAccess,
}

impl CfAuthProvider {
    pub fn new(config: Config) -> AuthResult<Self> {
        let access = CfAccess::new(&config.auth_team_base_url, &config.auth_audience)
            .map_err(AuthError::ServiceError)?;
        Ok(Self {
            access,
            team_base_url: config.auth_team_base_url,
        })
    }

    async fn validated_user_id(&self, token: &str) -> AuthResult<UserId> {
        self.access.validated_user_id(token).await
    }
}

#[derive(Deserialize, Clone)]
pub struct Config {
    /// `https://<team name>.cloudflareaccess.com`
    pub auth_team_base_url: String,
    /// Long hash from overview tab
    pub auth_audience: String,
}

#[async_trait]
impl AuthProvider for CfAuthProvider {
    type Config = Config;

    async fn healthcheck(&self) -> anyhow::Result<()> {
        self.access.refresh().await?;
        Ok(())
    }

    async fn register(&self, _username: &str) -> AuthResult<String> {
        Err(AuthError::Unimplemented)
    }

    async fn list_owners(&self, token: &str, _crate_name: &str) -> AuthResult<Vec<ListedOwner>> {
        self.validated_user_id(token).await?;

        Ok(vec![ListedOwner {
            id: 0,
            login: self.team_base_url.clone(),
            name: None,
        }])
    }

    async fn add_owners(&self, token: &str, _users: &[&str], _crate_name: &str) -> AuthResult<()> {
        // everyone is an owner, so it's technically a no-op
        self.validated_user_id(token).await?;
        Ok(())
    }

    async fn remove_owners(&self, token: &str, _users: &[&str], _crate_name: &str, ) -> AuthResult<()> {
        self.validated_user_id(token).await?;
        Err(AuthError::Unimplemented)
    }

    async fn publish(&self, token: &str, _crate_name: &str) -> AuthResult<()> {
        // only CI (using service token) is allowed to publish
        let id = self.validated_user_id(token).await?;
        if id.is_service_token() {
            Ok(())
        } else {
            Err(AuthError::Forbidden)
        }
    }

    async fn auth_yank(&self, token: &str, _crate_name: &str) -> AuthResult<()> {
        self.validated_user_id(token).await?;
        Ok(())
    }

    /// Fetch of config.json.
    async fn auth_config(&self, token: &str) -> AuthResult<()> {
        self.validated_user_id(token).await?;
        Ok(())
    }

    async fn auth_index_fetch(&self, token: &str, _all_users_can_read_crates: &str) -> AuthResult<()> {
        self.validated_user_id(token).await?;
        Ok(())
    }

    async fn auth_crate_download(&self, token: &str, _all_users_can_read_crates: &str) -> AuthResult<()> {
        self.validated_user_id(token).await?;
        Ok(())
    }

    async fn auth_view_full_index(&self, token: &str) -> AuthResult<()> {
        self.validated_user_id(token).await?;
        Ok(())
    }
}
