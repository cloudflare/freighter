pub mod utils;

use std::{
    collections::{BTreeMap, HashSet},
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
};

use async_trait::async_trait;
use axum::body::Bytes;
use freighter_auth::{AuthError, AuthProvider, AuthResult, ListedOwner};
use freighter_index::{
    CompletedPublication, CrateVersion, IndexError, IndexProvider, IndexResult, ListQuery, Publish,
    SearchResults, SearchResultsEntry,
};
use freighter_server::{ServiceConfig, ServiceState};
use freighter_storage::{StorageProvider, StorageResult};
use semver::Version;

#[derive(Default)]
pub struct MockIndexProvider {
    pub crates: BTreeMap<String, Vec<CrateVersion>>,
}

#[async_trait]
impl IndexProvider for MockIndexProvider {
    async fn get_sparse_entry(&self, crate_name: &str) -> IndexResult<Vec<CrateVersion>> {
        if let Some(versions) = self.crates.get(crate_name).cloned() {
            Ok(versions)
        } else {
            Err(IndexError::NotFound)
        }
    }
    async fn confirm_existence(&self, _crate_name: &str, _version: &Version) -> IndexResult<bool> {
        unimplemented!()
    }
    async fn yank_crate(&self, _crate_name: &str, _version: &Version) -> IndexResult<()> {
        unimplemented!()
    }
    async fn unyank_crate(&self, _crate_name: &str, _version: &Version) -> IndexResult<()> {
        unimplemented!()
    }
    async fn search(&self, _query_string: &str, _limit: usize) -> IndexResult<SearchResults> {
        unimplemented!()
    }
    async fn publish(
        &self,
        _version: &Publish,
        _checksum: &str,
        _end_step: Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>,
    ) -> IndexResult<CompletedPublication> {
        Ok(CompletedPublication { warnings: None })
    }

    async fn list(&self, _pagination: &ListQuery) -> IndexResult<Vec<SearchResultsEntry>> {
        Ok(self
            .crates
            .iter()
            .map(|(k, v)| {
                let max_version = v
                    .iter()
                    .max_by_key(|v| v.vers.clone())
                    .map(|v| v.vers.clone())
                    .unwrap();
                SearchResultsEntry {
                    name: k.clone(),
                    description: format!("Description {k} {max_version}"),
                    max_version,
                    homepage: Some("e.com".to_owned()),
                    repository: Some("ssh://git@b.com/a/f.git".to_owned()),
                    documentation: None,
                    keywords: vec!["example".to_owned()],
                    categories: vec!["a".to_owned(), "x".to_owned()],
                }
            })
            .collect())
    }
}

#[derive(Clone, Default)]
pub struct MockStorageProvider;

#[async_trait]
impl StorageProvider for MockStorageProvider {
    async fn pull_crate(&self, _name: &str, _version: &str) -> StorageResult<Bytes> {
        unimplemented!()
    }
    async fn put_crate(
        &self,
        _name: &str,
        _version: &str,
        _crate_bytes: &[u8],
    ) -> StorageResult<()> {
        unimplemented!()
    }
}

#[derive(Default)]
pub struct MockAuthProvider {
    pub valid_tokens: HashSet<String>,
}

#[async_trait]
impl AuthProvider for MockAuthProvider {
    async fn register(&self, _username: &str, _password: &str) -> AuthResult<String> {
        unimplemented!()
    }
    async fn login(&self, _username: &str, _password: &str) -> AuthResult<String> {
        unimplemented!()
    }
    async fn list_owners(&self, _token: &str, _crate_name: &str) -> AuthResult<Vec<ListedOwner>> {
        unimplemented!()
    }
    async fn add_owners(&self, _token: &str, _users: &[&str], _crate_name: &str) -> AuthResult<()> {
        unimplemented!()
    }
    async fn remove_owners(
        &self,
        _token: &str,
        _users: &[&str],
        _crate_name: &str,
    ) -> AuthResult<()> {
        unimplemented!()
    }
    async fn publish(&self, token: &str, _crate_name: &str) -> AuthResult<()> {
        if self.valid_tokens.contains(token) {
            Ok(())
        } else {
            Err(AuthError::Unauthorized)
        }
    }
    async fn auth_yank(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        unimplemented!()
    }
    async fn auth_unyank(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        unimplemented!()
    }
}

pub struct ServiceStateBuilder {
    config: ServiceConfig,
    index: MockIndexProvider,
    storage: MockStorageProvider,
    auth: MockAuthProvider,
}

impl Default for ServiceStateBuilder {
    fn default() -> Self {
        Self {
            config: ServiceConfig {
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000),
                download_endpoint: "localhost:4000".to_owned(),
                api_endpoint: "localhost:5000".to_owned(),
                metrics_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3001),
            },
            index: Default::default(),
            storage: Default::default(),
            auth: Default::default(),
        }
    }
}

impl ServiceStateBuilder {
    pub fn index_provider(mut self, provider: MockIndexProvider) -> Self {
        self.index = provider;
        self
    }

    pub fn storage_provider(mut self, provider: MockStorageProvider) -> Self {
        self.storage = provider;
        self
    }

    pub fn auth_provider(mut self, provider: MockAuthProvider) -> Self {
        self.auth = provider;
        self
    }

    pub fn build(
        self,
    ) -> Arc<ServiceState<MockIndexProvider, MockStorageProvider, MockAuthProvider>> {
        Arc::new(ServiceState {
            config: self.config,
            index: self.index,
            storage: self.storage,
            auth: self.auth,
        })
    }
}
