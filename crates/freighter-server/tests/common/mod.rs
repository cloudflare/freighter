pub mod utils;

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Bytes;
use freighter_api_types::index::request::{ListQuery, Publish};
use freighter_api_types::index::response::{
    CompletedPublication, CrateVersion, ListAll, ListAllCrateEntry, ListAllCrateVersion,
    SearchResults,
};
use freighter_api_types::index::{
    CrateVersionExists, IndexError, IndexProvider, IndexResult, SparseEntries,
};
use freighter_api_types::ownership::response::ListedOwner;
use freighter_api_types::storage::{FileResponse, StorageProvider, StorageResult};
use freighter_auth::{AuthError, AuthProvider, AuthResult};
use freighter_server::{ServiceConfig, ServiceState};
use semver::Version;

#[derive(Default)]
pub struct MockIndexProvider {
    pub crates: BTreeMap<String, Vec<CrateVersion>>,
}

#[async_trait]
impl IndexProvider for MockIndexProvider {
    type Config = ();

    async fn healthcheck(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_sparse_entry(&self, crate_name: &str) -> IndexResult<SparseEntries> {
        if let Some(entries) = self.crates.get(crate_name).cloned() {
            Ok(SparseEntries { entries, last_modified: None })
        } else {
            Err(IndexError::NotFound)
        }
    }
    async fn confirm_existence(
        &self,
        _crate_name: &str,
        _version: &Version,
    ) -> IndexResult<CrateVersionExists> {
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
        _checksum: [u8; 32],
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
    async fn pull_crate(
        &self,
        _name: &str,
        _version: &str,
        _digest: [u8; 32],
    ) -> StorageResult<FileResponse> {
        unimplemented!()
    }

    async fn put_crate(
        &self,
        _name: &str,
        _version: &str,
        _crate_bytes: Bytes,
        _sha256: [u8; 32],
    ) -> StorageResult<()> {
        Ok(())
    }

    async fn delete_crate(
        &self,
        _name: &str,
        _version: &str,
        _digest: [u8; 32],
    ) -> StorageResult<()> {
        Ok(())
    }

    async fn healthcheck(&self) -> anyhow::Result<()> {
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
            Err(AuthError::Forbidden)
        }
    }
    async fn auth_yank(&self, _token: &str, _crate_name: &str) -> AuthResult<()> {
        unimplemented!()
    }

    async fn auth_view_full_index(&self, token: &str) -> AuthResult<()> {
        if self.valid_tokens.contains(token) {
            Ok(())
        } else {
            Err(AuthError::Forbidden)
        }
    }

    async fn auth_config(&self, token: &str) -> AuthResult<()> {
        if self.valid_tokens.contains(token) {
            Ok(())
        } else {
            Err(AuthError::Forbidden)
        }
    }

    async fn healthcheck(&self) -> anyhow::Result<()> {
        Ok(())
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
                auth_required: false,
                crate_size_limit: 1024 * 1024,
            },
            index: Default::default(),
            storage: Default::default(),
            auth: Default::default(),
        }
    }
}

impl ServiceStateBuilder {
    #[must_use]
    pub fn index_provider(mut self, provider: MockIndexProvider) -> Self {
        self.index = provider;
        self
    }

    #[must_use]
    pub fn storage_provider(mut self, provider: MockStorageProvider) -> Self {
        self.storage = provider;
        self
    }

    #[must_use]
    pub fn auth_provider(mut self, provider: MockAuthProvider) -> Self {
        self.auth = provider;
        self
    }

    #[must_use]
    pub fn auth_required(mut self, req: bool) -> Self {
        self.config.auth_required = req;
        self
    }

    #[must_use]
    pub fn build(self) -> Arc<ServiceState> {
        Arc::new(ServiceState {
            config: self.config,
            index: Box::new(self.index),
            storage: Box::new(self.storage),
            auth: Box::new(self.auth),
        })
    }

    #[must_use]
    pub fn build_no_arc(self) -> ServiceState {
        ServiceState {
            config: self.config,
            index: Box::new(self.index),
            storage: Box::new(self.storage),
            auth: Box::new(self.auth),
        }
    }
}
