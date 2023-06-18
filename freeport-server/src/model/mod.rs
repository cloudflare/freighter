use crate::config::Config;
use axum::body::Bytes;

use s3::Bucket;

pub struct ServiceState<I> {
    pub config: Config,
    pub index: I,
    bucket: Bucket,
}

impl<I> ServiceState<I> {
    pub fn new(config: Config, index: I) -> Self {
        let bucket = Bucket::new(
            &config.store.name,
            config.store.region.clone(),
            config.store.credentials.clone(),
        )
        .unwrap();

        Self {
            config,
            index,
            bucket,
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

    pub async fn download_crate(&self, name: &str) -> Option<Bytes> {
        self.bucket
            .get_object(name)
            .await
            .ok()
            .map(|x| x.bytes().clone())
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
