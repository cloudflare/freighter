//! Storage backend implementation for working with bucketing solutions compatible with the S3 API.
//!
//! This is currently built on the [`s3`] crate.
//!
//! It also currently has several known performance limitations, some associated with that crate.
//! These limitations will all be addressed in the future.
//!
//! In particular, it does no connection pooling whatsoever.
//! That crate does not appear to persist the hyper client between requests, so 2-3 RTTs of latency
//! will be incurred for every API call, plus additional delays due to TCP warmup.
//!
//! In the future, this should be changed to use the [aws-sdk-s3] crate instead.
//!
//! Another performance limitation of this implementation is that the current simplistic API of
//! this crate ([`StorageClient`]) does not allow for streamed uploads or downloads, increasing
//! both memory usage and latency, as the entire body of data must be received before transmission
//! can start.
//! This means that when uploading, the server must wait for and store the entirety of the HTTP
//! request before it can begin uploading the body to the bucket, and, when downloading, the
//! entirety of the response from the bucket must be received before transmission of the crate
//! bytes to the eyeball.
//! It is perfectly possible to perform both streaming uploads and streaming downloads, even with
//! the [`s3`] crate, however doing so will probably be left to whenever this is switched over to
//! use the [aws-sdk-s3] crate.
//!
//! [aws-sdk-s3]: https://docs.rs/aws-sdk-s3/latest/aws_sdk_s3/

use crate::{StorageClient, StorageError, StorageResult};
use anyhow::Context;
use async_trait::async_trait;
use bytes::Bytes;
use exports::awsregion::Region;
use exports::creds::Credentials;
use s3::error::S3Error;
use s3::Bucket;

/// Re-exports needed for interacting with the [`S3StorageClient`](super::S3StorageClient).
pub mod exports {
    pub use awsregion;
    pub use s3::creds;
}

/// Storage client for working with S3-compatible APIs.
///
/// See [the module-level docs](super::s3_client) for more information.
#[derive(Clone)]
pub struct S3StorageClient {
    bucket: Bucket,
}

impl S3StorageClient {
    /// Construct a new client, returning an error if the information could not be used to
    /// communicate with a valid bucket.
    pub fn new(name: &str, region: Region, credentials: Credentials) -> StorageResult<Self> {
        let bucket = Bucket::new(name, region, credentials).context("Failed to create bucket")?;

        Ok(Self { bucket })
    }
}

#[async_trait]
impl StorageClient for S3StorageClient {
    async fn pull_crate(&self, name: &str, version: &str) -> StorageResult<Bytes> {
        let path = construct_path(name, version);

        let resp = self.bucket.get_object(path).await;

        // on 404, we return a different error variant
        if let Err(S3Error::Http(code, _)) = &resp {
            if *code == 404 {
                return Err(StorageError::NotFound);
            }
        }

        let data = resp.context("Failed to retrieve crate")?;

        let response_bytes_vec: Vec<u8> = data.into();

        Ok(Bytes::from(response_bytes_vec))
    }

    async fn put_crate(&self, name: &str, version: &str, crate_bytes: &[u8]) -> StorageResult<()> {
        let path = construct_path(name, version);

        self.bucket
            .put_object(path, crate_bytes)
            .await
            .context("Failed to put crate in bucket")?;

        Ok(())
    }
}

#[inline(always)]
fn construct_path(name: &str, version: &str) -> String {
    format!("{name}-{version}.crate")
}
