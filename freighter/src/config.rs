use freighter_server::ServiceConfig;
use freighter_storage::s3_client::exports::{awsregion, creds};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    pub db: deadpool_postgres::Config,
    pub store: StoreConfig,
}

#[derive(Deserialize)]
pub struct StoreConfig {
    pub name: String,
    pub region: awsregion::Region,
    pub credentials: creds::Credentials,
}
