use std::path::PathBuf;
use freighter_server::ServiceConfig;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    pub index_db: Option<deadpool_postgres::Config>,
    pub index_path: Option<PathBuf>,
    pub auth_db: deadpool_postgres::Config,
    pub store: StoreConfig,
}

#[derive(Deserialize)]
pub struct StoreConfig {
    pub name: String,
    pub endpoint_url: String,
    pub region: String,
    pub access_key_id: String,
    pub access_key_secret: String,
}
