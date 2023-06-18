use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Clone, Deserialize)]
pub struct ServiceConfig {
    pub address: SocketAddr,
    pub download_endpoint: String,
    pub api_endpoint: String,
    pub metrics_address: SocketAddr,
}
