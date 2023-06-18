use crate::config::Config;

pub struct ServiceState<I, S> {
    pub config: Config,
    pub index: I,
    pub storage: S,
}

impl<I, S> ServiceState<I, S> {
    pub fn new(config: Config, index: I, storage: S) -> Self {
        Self {
            config,
            index,
            storage,
        }
    }

    pub async fn auth_user_action(&self, _token: &str, _crate_name: &str) -> bool {
        todo!()
        // let client = self.pool.get().await.unwrap();
        //
        // let statement = client
        //     .prepare_cached(include_str!("../../sql/auth-crate-action.sql"))
        //     .await
        //     .unwrap();
        //
        // client
        //     .query_one(&statement, &[&token, &crate_name])
        //     .await
        //     .is_ok()
    }

    pub async fn register(&self, _username: &str, _password: &str) -> Option<String> {
        todo!()
        // let client = self.pool.get().await.unwrap();
        //
        // let statement = client
        //     .prepare_cached(include_str!("../../sql/register.sql"))
        //     .await
        //     .unwrap();
        //
        // client
        //     .query_one(&statement, &[&username, &password])
        //     .await
        //     .unwrap();
        //
        // self.login(username, password).await
    }

    pub async fn login(&self, _username: &str, _password: &str) -> Option<String> {
        todo!()
        // let client = self.pool.get().await.unwrap();
        //
        // let statement = client
        //     .prepare_cached(include_str!("../../sql/login.sql"))
        //     .await
        //     .unwrap();
        //
        // let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
        //
        // if client
        //     .query_one(&statement, &[&username, &password, &token])
        //     .await
        //     .is_ok()
        // {
        //     Some(token)
        // } else {
        //     None
        // }
    }
}
