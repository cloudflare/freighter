use freighter_api_types::index::IndexProvider;
use freighter_auth::AuthProvider;
use freighter_server::ServiceConfig;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config<I: IndexProvider, A: AuthProvider> {
    pub service: ServiceConfig,
    #[serde(flatten)]
    pub index_config: I::Config,
    #[serde(flatten)]
    pub auth_config: A::Config,
    pub store: StoreConfig,
}

#[derive(Deserialize)]
pub struct StoreConfig {
    pub name: String,
    pub endpoint_url: String,
    pub region: String,
    pub access_key_id: Option<String>,
    pub access_key_secret: Option<String>,
}
