use crate::{AuthError, AuthProvider, AuthResult, ListedOwner};
use anyhow::Context;
use async_trait::async_trait;
use deadpool_postgres::tokio_postgres::NoTls;
use deadpool_postgres::{GenericClient, Pool, Runtime};
use rand::distributions::{Alphanumeric, DistString};

pub struct PgAuthProvider {
    pool: Pool,
}

impl PgAuthProvider {
    pub fn new(config: deadpool_postgres::Config) -> AuthResult<Self> {
        let pool = config
            .create_pool(Some(Runtime::Tokio1), NoTls)
            .context("Failed to create auth db pool")?;

        Ok(Self { pool })
    }

    async fn auth_crate_action(&self, token: &str, crate_name: &str) -> AuthResult<()> {
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
                0 => Err(AuthError::InvalidCredentials),
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

#[async_trait]
impl AuthProvider for PgAuthProvider {
    async fn register(&self, username: &str, password: &str) -> AuthResult<String> {
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
            .query_one(&register_statement, &[&username, &password])
            .await
            .context("Failed to register user")?;

        let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

        transaction
            .query_one(&login_statement, &[&username, &password, &token])
            .await
            .context("Failed to login user after registering")?;

        transaction
            .commit()
            .await
            .context("Failed to commit registration transaction")?;

        Ok(token)
    }

    async fn login(&self, username: &str, password: &str) -> AuthResult<String> {
        let client = self
            .pool
            .get()
            .await
            .context("Failed to get auth db client from pool")?;

        let login_statement = client
            .prepare_cached(include_str!("../sql/login.sql"))
            .await
            .context("Failed to prepare login statement")?;

        let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

        client
            .query_one(&login_statement, &[&username, &password, &token])
            .await
            .context("Failed to login user")?;

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

    async fn auth_unyank(&self, token: &str, crate_name: &str) -> AuthResult<()> {
        self.auth_crate_action(token, crate_name).await
    }
}
