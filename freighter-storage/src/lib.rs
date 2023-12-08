#![cfg_attr(docsrs, feature(doc_cfg))]

use async_trait::async_trait;
use bytes::Bytes;

#[cfg(feature = "s3-backend")]
#[cfg_attr(docsrs, doc(cfg(feature = "s3-backend")))]
pub mod s3_client;

mod error;

pub use error::*;

#[async_trait]
pub trait StorageProvider {
    async fn pull_crate(&self, name: &str, version: &str) -> StorageResult<Bytes>;
    async fn put_crate(&self, name: &str, version: &str, crate_bytes: Bytes) -> StorageResult<()>;
    /// Called to undo a put after a failed index transaction
    async fn delete_crate(&self, name: &str, version: &str) -> StorageResult<()>;
}
