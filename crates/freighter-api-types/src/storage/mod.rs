pub use bytes::Bytes;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

pub use error::{StorageError, StorageResult};

mod error;

#[async_trait]
pub trait StorageProvider {
    async fn pull_crate(&self, name: &str, version: &str) -> StorageResult<FileResponse>;
    async fn put_crate(
        &self,
        name: &str,
        version: &str,
        crate_bytes: Bytes,
        sha256: [u8; 32],
    ) -> StorageResult<()>;
    /// Called to undo a put after a failed index transaction
    async fn delete_crate(&self, name: &str, version: &str) -> StorageResult<()>;

    async fn healthcheck(&self) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub content_type: Option<&'static str>,
    pub content_length: Option<usize>,
    pub cache_control: Option<String>,
    pub content_encoding: Option<String>,
    pub sha256: Option<[u8; 32]>,
    pub kv: HashMap<String, String>,
}

pub struct FileResponse {
    pub last_modified: Option<DateTime<Utc>>,
    pub data: Bytes,
}

#[async_trait]
pub trait MetadataStorageProvider {
    async fn pull_file(&self, path: &str) -> StorageResult<FileResponse>;
    async fn put_file(&self, path: &str, file_bytes: Bytes, meta: Metadata) -> StorageResult<()>;
    async fn delete_file(&self, path: &str) -> StorageResult<()>;
    async fn list_prefix(&self, path: &str) -> StorageResult<Vec<String>>;
    async fn healthcheck(&self) -> anyhow::Result<()>;
}
