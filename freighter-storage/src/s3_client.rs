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

use crate::{StorageError, StorageProvider, StorageResult};
use anyhow::Context;
use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::{AppName, Config, Region, BehaviorVersion};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;

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
            .behavior_version(BehaviorVersion::v2023_11_09())
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
}

#[async_trait]
impl StorageProvider for S3StorageProvider {
    async fn pull_crate(&self, name: &str, version: &str) -> StorageResult<Bytes> {
        let path = construct_path(name, version);

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

        let data = resp.context("Failed to retrieve crate")?;

        let crate_bytes = data
            .body
            .collect()
            .await
            .context("Failed to retrieve body of crate")?
            .into_bytes();

        Ok(crate_bytes)
    }

    async fn put_crate(&self, name: &str, version: &str, crate_bytes: &[u8]) -> StorageResult<()> {
        let path = construct_path(name, version);

        self.client
            .put_object()
            .bucket(self.bucket_name.clone())
            .key(path)
            .body(ByteStream::from(crate_bytes.to_vec()))
            .send()
            .await
            .context("Failed to put crate in bucket")?;

        Ok(())
    }

    async fn delete_crate(&self, name: &str, version: &str) -> StorageResult<()> {
        let path = construct_path(name, version);

        self.client
            .delete_object()
            .bucket(self.bucket_name.clone())
            .key(path)
            .send()
            .await
            .context("Failed to delete a crate from a bucket")?;

        Ok(())
    }
}

#[inline(always)]
fn construct_path(name: &str, version: &str) -> String {
    format!("{name}-{version}.crate")
}
