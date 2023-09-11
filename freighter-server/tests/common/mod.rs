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
use freighter_api_types::index::request::{ListQuery, Publish};
use freighter_api_types::index::response::{
    CompletedPublication, CrateVersion, ListAll, ListAllCrateEntry, ListAllCrateVersion,
    SearchResults,
};
use freighter_api_types::index::{IndexError, IndexProvider, IndexResult};
use freighter_api_types::ownership::response::ListedOwner;
use freighter_auth::{AuthError, AuthProvider, AuthResult};
use freighter_server::{ServiceConfig, ServiceState};
use freighter_storage::{StorageProvider, StorageResult};
use semver::Version;

#[derive(Default)]
pub struct MockIndexProvider {
    pub crates: BTreeMap<String, Vec<CrateVersion>>,
}

#[async_trait]
impl IndexProvider for MockIndexProvider {
    type Config = ();

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
        end_step: Pin<&mut (dyn Future<Output = IndexResult<()>> + Send)>,
    ) -> IndexResult<CompletedPublication> {
        end_step.await?;
        Ok(CompletedPublication { warnings: None })
    }

    async fn list(&self, _pagination: &ListQuery) -> IndexResult<ListAll> {
        let crates = self
            .crates
            .iter()
            .map(|(k, v)| {
                let versions = v
                    .iter()
                    .map(|v| ListAllCrateVersion {
                        version: v.vers.clone(),
                    })
                    .collect();

                ListAllCrateEntry {
                    name: k.clone(),
                    description: format!("Description {k}"),
                    created_at: Default::default(),
                    updated_at: Default::default(),
                    versions,
                    homepage: Some("e.com".to_owned()),
                    repository: Some("ssh://git@b.com/a/f.git".to_owned()),
                    documentation: None,
                    keywords: vec!["example".to_owned()],
                    categories: vec!["a".to_owned(), "x".to_owned()],
                }
            })
            .collect();

        Ok(ListAll { results: crates })
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
        Ok(())
    }
    async fn delete_crate(&self, _name: &str, _version: &str) -> StorageResult<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MockAuthProvider {
    pub valid_tokens: HashSet<String>,
}

#[async_trait]
impl AuthProvider for MockAuthProvider {
    type Config = ();

    async fn register(&self, _username: &str) -> AuthResult<String> {
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
}

pub struct ServiceStateBuilder {
    pub config: ServiceConfig,
    pub index: MockIndexProvider,
    pub storage: MockStorageProvider,
    pub auth: MockAuthProvider,
}

impl Default for ServiceStateBuilder {
    fn default() -> Self {
        Self {
            config: ServiceConfig {
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000),
                download_endpoint: "localhost:4000".to_owned(),
                api_endpoint: "localhost:5000".to_owned(),
                metrics_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3001),
                allow_registration: true,
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

    pub fn build_no_arc(
        self,
    ) -> ServiceState<MockIndexProvider, MockStorageProvider, MockAuthProvider> {
        ServiceState {
            config: self.config,
            index: self.index,
            storage: self.storage,
            auth: self.auth,
        }
    }
}
