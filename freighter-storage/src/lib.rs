#![cfg_attr(docsrs, feature(doc_cfg))]

use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;

#[cfg(feature = "s3-backend")]
#[cfg_attr(docsrs, doc(cfg(feature = "s3-backend")))]
pub mod s3_client;

mod error;

pub use error::*;

#[async_trait]
pub trait StorageProvider {
    async fn pull_crate(&self, name: &str, version: &str) -> StorageResult<Bytes>;
    async fn put_crate(&self, name: &str, version: &str, crate_bytes: Bytes, sha256: [u8; 32]) -> StorageResult<()>;
    /// Called to undo a put after a failed index transaction
    async fn delete_crate(&self, name: &str, version: &str) -> StorageResult<()>;
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
