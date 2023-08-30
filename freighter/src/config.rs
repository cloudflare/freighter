use freighter_api_types::index::IndexProvider;
use freighter_server::ServiceConfig;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config<I: IndexProvider> {
    pub service: ServiceConfig,
    #[serde(flatten)]
    pub index_config: I::Config,
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
