//! Storage backend implementation for working with bucketing solutions compatible with the S3 API.
//!
//! This is currently built on the [`aws_sdk_s3`] crate.
//!
//! This client should do connection pooling, however the HTTP connection pool parameters not not
//! well tuned at the moment.
//!
//! One performance limitation of this implementation is that the current simplistic API of this
//! crate ([`StorageProvider`]) does not allow for streamed uploads or downloads, increasing both
//! memory usage and latency, as the entire body of data must be received before transmission can
//! start.
//! This means that when uploading, the server must wait for and store the entirety of the HTTP
//! request before it can begin uploading the body to the bucket, and, when downloading, the
//! entirety of the response from the bucket must be received before transmission of the crate
//! bytes to the eyeball.
//! It is perfectly possible to perform both streaming uploads and streaming downloads, however
//! doing so has been left to the future.

use anyhow::{bail, Context};
use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::{AppName, BehaviorVersion, Config, Region};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use freighter_api_types::storage::{
    Metadata, MetadataStorageProvider, StorageError, StorageProvider, StorageResult,
};
use std::collections::HashMap;

/// Storage client for working with S3-compatible APIs.
///
/// See [the module-level docs](super::s3_client) for more information.
#[derive(Clone)]
pub struct S3StorageProvider {
    client: aws_sdk_s3::Client,
    bucket_name: String,
}

impl S3StorageProvider {
    /// Construct a new client, returning an error if the information could not be used to
    /// communicate with a valid bucket.
    #[must_use]
    pub fn new(
        bucket_name: &str,
        endpoint_url: &str,
        region: &str,
        access_key: &str,
        secret_key: &str,
    ) -> Self {
        let config = Config::builder()
            .behavior_version(BehaviorVersion::v2024_03_28())
            .region(Region::new(region.to_string()))
            .endpoint_url(endpoint_url)
            .credentials_provider(Credentials::from_keys(access_key, secret_key, None))
            .app_name(AppName::new("freighter".to_string()).unwrap())
            .build();

        let bucket_name = bucket_name.to_string();
        let client = aws_sdk_s3::Client::from_conf(config);

        Self {
            client,
            bucket_name,
        }
    }

    async fn pull_object(&self, path: String) -> StorageResult<Bytes> {
        let resp = self
            .client
            .get_object()
            .bucket(self.bucket_name.clone())
            .key(path)
            .send()
            .await;

        // on 404, we return a different error variant
        if let Err(SdkError::ServiceError(e)) = &resp {
            if e.err().is_no_such_key() {
                return Err(StorageError::NotFound);
            }
        }

        let data = resp.context("Storage response error")?;

        let crate_bytes = data
            .body
            .collect()
            .await
            .context("Error while retrieving body")?
            .into_bytes();

        Ok(crate_bytes)
    }

    async fn put_object(&self, path: String, file_bytes: ByteStream, meta: Metadata) -> StorageResult<()> {
        let mut obj = self.client
            .put_object()
            .bucket(self.bucket_name.clone())
            .key(path)
            .body(file_bytes);
        if let Some(len) = meta.content_length {
            obj = obj.content_length(len as _);
        }
        if let Some(ty) = meta.content_type {
            obj = obj.content_type(ty);
        }
        if let Some(ty) = meta.content_encoding {
            obj = obj.content_encoding(ty);
        }
        if let Some(cc) = meta.cache_control {
            obj = obj.cache_control(cc);
        }
        if let Some(sha) = meta.sha256 {
            use base64::{engine, Engine as _};
            obj = obj.checksum_sha256(engine::general_purpose::STANDARD.encode(sha));
        }
        for (k, v) in meta.kv {
            obj = obj.metadata(k, v);
        }
        obj.send().await.context("Failed to put file")?;
        Ok(())
    }

    async fn delete_object(&self, path: String) -> StorageResult<()> {
        self.client
            .delete_object()
            .bucket(self.bucket_name.clone())
            .key(path)
            .send()
            .await
            .context("Failed to delete file")?;
        Ok(())
    }

    async fn healthcheck(&self, path: String) -> Result<(), anyhow::Error> {
        self.put_object(path.clone(), Bytes::from_static(b"ok").into(), Metadata {
            content_type: Some("text/plain"),
            ..Metadata::default()
        }).await?;
        if self.pull_object(path).await? != b"ok"[..] {
            bail!("wrong data");
        }
        Ok(())
    }
}

#[async_trait]
impl MetadataStorageProvider for S3StorageProvider {
    async fn pull_file(&self, path: &str) -> StorageResult<Bytes> {
        self.pull_object(path.into()).await
    }

    async fn put_file(&self, path: &str, file_bytes: Bytes, meta: Metadata) -> StorageResult<()> {
        self.put_object(path.into(), file_bytes.into(), meta).await
    }

    async fn create_or_append_file(&self, path: &str, file_bytes: Bytes, meta: Metadata) -> StorageResult<()> {
        let mut all_data = match self.pull_object(path.into()).await {
            Ok(data) => Vec::from(data),
            Err(StorageError::NotFound) => Vec::new(),
            Err(e) => return Err(e),
        };
        all_data.append(&mut Vec::from(file_bytes));
        self.put_object(path.into(), all_data.into(), meta).await
    }

    async fn delete_file(&self, path: &str) -> StorageResult<()> {
        self.delete_object(path.into()).await
    }

    async fn healthcheck(&self) -> anyhow::Result<()> {
        self.healthcheck(".healthcheck-meta".into()).await
    }
}

#[async_trait]
impl StorageProvider for S3StorageProvider {
    async fn pull_crate(&self, name: &str, version: &str) -> StorageResult<Bytes> {
        let path = construct_path(name, version);
        self.pull_object(path).await
    }

    async fn put_crate(&self, name: &str, version: &str, crate_bytes: Bytes, sha256: [u8; 32]) -> StorageResult<()> {
        let len = crate_bytes.len();
        let path = construct_path(name, version);
        self.put_object(path, crate_bytes.into(), Metadata {
            content_type: Some("application/x-tar"),
            content_length: Some(len),
            cache_control: Some("public,immutable".into()),
            content_encoding: Some("gzip".into()),
            sha256: Some(sha256),
            kv: HashMap::new(),
        }).await
    }

    async fn delete_crate(&self, name: &str, version: &str) -> StorageResult<()> {
        let path = construct_path(name, version);
        self.delete_object(path).await
    }

    async fn healthcheck(&self) -> anyhow::Result<()> {
        self.healthcheck(".healthcheck-data".into()).await
    }
}

#[inline(always)]
fn construct_path(name: &str, version: &str) -> String {
    format!("{name}-{version}.crate")
}
