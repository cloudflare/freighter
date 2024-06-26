use crate::{AuthError, AuthProvider, AuthResult};
use anyhow::Context;
use async_trait::async_trait;
use deadpool_postgres::tokio_postgres::NoTls;
use deadpool_postgres::{GenericClient, Pool, Runtime};
use freighter_api_types::ownership::response::ListedOwner;
use rand::distributions::{Alphanumeric, DistString};
use serde::Deserialize;

const TOKEN_LENGTH: usize = 32;

pub struct PgAuthProvider {
    pool: Pool,
}

impl PgAuthProvider {
    pub fn new(config: Config) -> AuthResult<Self> {
        let pool = config
            .auth_db
            .create_pool(Some(Runtime::Tokio1), NoTls)
            .context("Failed to create auth db pool")?;

        Ok(Self { pool })
    }

    async fn auth_crate_action(&self, token: &str, crate_name: &str) -> AuthResult<()> {
        if token.len() != TOKEN_LENGTH {
            return Err(AuthError::InvalidCredentials);
        }

        let client = self
            .pool
            .get()
            .await
            .context("Failed to get auth db client from pool")?;

        let auth_statement = client
            .prepare_cached(include_str!("../sql/auth-crate-action.sql"))
            .await
            .context("Failed to prepare auth statement")?;

        client
            .query(&auth_statement, &[&token, &crate_name])
            .await
            .context("Failed to auth crate action")
            .map_err(AuthError::ServiceError)
            .and_then(|r| match r.len() {
                0 => Err(AuthError::Unauthorized),
                1 => Ok(()),
                _ => Err(AuthError::ServiceError(anyhow::anyhow!(
                    "Unexpected number of rows"
                ))),
            })?;

        Ok(())
    }

    async fn list_owners_no_auth(&self, crate_name: &str) -> AuthResult<Vec<ListedOwner>> {
        let client = self
            .pool
            .get()
            .await
            .context("Failed to get auth db client from pool")?;

        let statement = client
            .prepare_cached(include_str!("../sql/list-owners.sql"))
            .await
            .context("Failed to prepare owners list statement")?;

        let owners = client
            .query(&statement, &[&crate_name])
            .await
            .context("Failed to auth crate transaction")?
            .into_iter()
            .map(|row| ListedOwner {
                id: row.get::<_, i32>("id") as u32,
                login: row.get("username"),
                name: None,
            })
            .collect();

        Ok(owners)
    }

    async fn add_owners_no_auth(&self, users: &[&str], crate_name: &str) -> AuthResult<()> {
        let mut client = self
            .pool
            .get()
            .await
            .context("Failed to get auth db client from pool")?;

        let transaction = client
            .transaction()
            .await
            .context("Failed to construct transaction for adding owners")?;

        let (get_id_statement, add_owner_statement) = tokio::try_join!(
            transaction.prepare_cached(include_str!("../sql/get-user-id.sql")),
            transaction.prepare_cached(include_str!("../sql/add-owner.sql"))
        )
        .context("Failed to prepare statements for adding owners")?;

        // this is like basically the worst possible way of doing this, but this command is such a
        // cold path that we do not care.
        //
        // ideally this would be pipelined heavily via a pair of FuturesUnordered
        //
        // this is theoretically perfect for pipelining, but also pointless to pipeline
        for &user in users {
            let user_id: i32 = transaction
                .query_one(&get_id_statement, &[&user])
                .await
                .context("Failed to fetch user id")?
                .get("id");

            transaction
                .query(&add_owner_statement, &[&user_id, &crate_name])
                .await
                .context("Failed to add owner to crate")?;
        }

        transaction
            .commit()
            .await
            .context("Failed to commit add owners transaction")?;

        Ok(())
    }

    async fn remove_owners_no_auth(&self, users: &[&str], crate_name: &str) -> AuthResult<()> {
        let client = self
            .pool
            .get()
            .await
            .context("Failed to get auth db client from pool")?;

        let statement = client
            .prepare_cached(include_str!("../sql/remove-owner.sql"))
            .await
            .context("Failed to prepare remove owner statement")?;

        // this is like basically the worst possible way of doing this, but this command is such a
        // cold path that we do not care.
        for &user in users {
            client
                .query_one(&statement, &[&user, &crate_name])
                .await
                .context("Failed to remove user from crate ownership")?;
        }

        Ok(())
    }

    async fn get_user_for_token(&self, token: &str) -> AuthResult<String> {
        if token.len() != TOKEN_LENGTH {
            return Err(AuthError::InvalidCredentials);
        }

        let client = self
            .pool
            .get()
            .await
            .context("Failed to get auth db client from pool")?;

        let statement = client
            .prepare_cached(include_str!("../sql/get-user-for-token.sql"))
            .await
            .context("Failed to prepare get token user statement")?;

        let user = client
            .query_one(&statement, &[&token])
            .await
            .context("Failed to query for user of token")?
            .get("username");

        Ok(user)
    }
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub auth_db: deadpool_postgres::Config,
}

#[async_trait]
impl AuthProvider for PgAuthProvider {
    type Config = Config;

    async fn healthcheck(&self) -> anyhow::Result<()> {
        let _ = self.pool.get().await?;
        Ok(())
    }

    async fn register(&self, username: &str) -> AuthResult<String> {
        let mut client = self
            .pool
            .get()
            .await
            .context("Failed to get auth db client from pool")?;

        // conduct a transaction, just in case something happens midway through.
        // it would be very confusing for users if they ended up registered but not logged in.
        let transaction = client
            .transaction()
            .await
            .context("Failed to create registration transaction")?;

        let (register_statement, login_statement) = tokio::try_join!(
            transaction.prepare_cached(include_str!("../sql/register.sql")),
            transaction.prepare_cached(include_str!("../sql/login.sql")),
        )
        .context("Failed to prepare statements for registering user")?;

        transaction
            .query_one(&register_statement, &[&username])
            .await
            .context("Failed to register user")?;

        let token = Alphanumeric.sample_string(&mut rand::thread_rng(), TOKEN_LENGTH);

        transaction
            .query_one(&login_statement, &[&username, &token])
            .await
            .context("Failed to login user after registering")?;

        transaction
            .commit()
            .await
            .context("Failed to commit registration transaction")?;

        Ok(token)
    }

    async fn list_owners(&self, token: &str, crate_name: &str) -> AuthResult<Vec<ListedOwner>> {
        self.auth_crate_action(token, crate_name).await?;

        self.list_owners_no_auth(crate_name).await
    }

    async fn add_owners(&self, token: &str, users: &[&str], crate_name: &str) -> AuthResult<()> {
        self.auth_crate_action(token, crate_name).await?;

        self.add_owners_no_auth(users, crate_name).await
    }

    async fn remove_owners(&self, token: &str, users: &[&str], crate_name: &str) -> AuthResult<()> {
        self.auth_crate_action(token, crate_name).await?;

        self.remove_owners_no_auth(users, crate_name).await
    }

    async fn publish(&self, token: &str, crate_name: &str) -> AuthResult<()> {
        let crate_owners = self.list_owners_no_auth(crate_name).await?;

        if crate_owners.is_empty() {
            let user = self.get_user_for_token(token).await?;

            self.add_owners_no_auth(&[&user], crate_name).await?;

            Ok(())
        } else {
            self.auth_crate_action(token, crate_name).await
        }
    }

    async fn auth_yank(&self, token: &str, crate_name: &str) -> AuthResult<()> {
        self.auth_crate_action(token, crate_name).await
    }

    async fn auth_config(&self, token: &str) -> AuthResult<()> {
        let _ = self.get_user_for_token(token).await?;
        Ok(())
    }

    async fn auth_index_fetch(&self, token: &str, _all_users_can_read_crates: &str) -> AuthResult<()> {
        let _ = self.get_user_for_token(token).await?;
        Ok(())
    }

    async fn auth_crate_download(&self, token: &str, _all_users_can_read_crates: &str) -> AuthResult<()> {
        let _ = self.get_user_for_token(token).await?;
        Ok(())
    }

    async fn auth_view_full_index(&self, token: &str) -> AuthResult<()> {
        let _ = self.get_user_for_token(token).await?;
        Ok(())
    }
}
