use s3::creds::Credentials;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    pub db: deadpool_postgres::Config,
    pub store: StoreConfig,
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub address: SocketAddr,
    pub download_endpoint: String,
    pub api_endpoint: String,
    pub metrics_address: SocketAddr,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoreConfig {
    pub name: String,
    pub region: awsregion::Region,
    pub credentials: Credentials,
}
