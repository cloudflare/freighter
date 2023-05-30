use std::collections::HashMap;
use std::time::Duration;

use crate::bucket_ops::{BucketConfiguration, CreateBucketResponse};
use crate::command::{Command, Multipart};
use crate::creds::Credentials;
use crate::region::Region;
use crate::request::ResponseData;
use std::str::FromStr;
use std::sync::{Arc, RwLock};

pub type Query = HashMap<String, String>;

use crate::request::tokio_backend::HyperRequest as RequestImpl;

use tokio::io::AsyncWrite;

use std::io::Read;

use tokio::io::AsyncRead;

use crate::error::S3Error;
use crate::serde_types::{
    BucketLocationResult, CompleteMultipartUploadData, CorsConfiguration, HeadObjectResult,
    InitiateMultipartUploadResponse, ListBucketResult, ListMultipartUploadsResult, Part,
};
use crate::utils::{error_from_response_data, PutStreamResponse};
use hyper::http::header::HeaderName;
use hyper::http::HeaderMap;

pub const CHUNK_SIZE: usize = 8_388_608; // 8 Mebibytes, min is 5 (5_242_880);

const DEFAULT_REQUEST_TIMEOUT: Option<Duration> = Some(Duration::from_secs(60));

#[derive(Debug, PartialEq, Eq)]
pub struct Tag {
    key: String,
    value: String,
}

impl Tag {
    pub fn key(&self) -> String {
        self.key.to_owned()
    }

    pub fn value(&self) -> String {
        self.value.to_owned()
    }
}

#[derive(Clone, Debug)]
pub struct Bucket {
    pub name: String,
    pub region: Region,
    pub credentials: Arc<RwLock<Credentials>>,
    pub extra_headers: HeaderMap,
    pub extra_query: Query,
    pub request_timeout: Option<Duration>,
    path_style: bool,
    listobjects_v2: bool,
}

impl Bucket {
    pub fn credentials_refresh(&self) -> Result<(), S3Error> {
        Ok(self
            .credentials
            .try_write()
            .map_err(|_| S3Error::WLCredentials)?
            .refresh()?)
    }
}

fn validate_expiry(expiry_secs: u32) -> Result<(), S3Error> {
    if 604800 < expiry_secs {
        return Err(S3Error::MaxExpiry(expiry_secs));
    }
    Ok(())
}

#[cfg_attr(all(feature = "with-tokio", feature = "blocking"), block_on("tokio"))]
#[cfg_attr(
    all(feature = "with-async-std", feature = "blocking"),
    block_on("async-std")
)]
impl Bucket {
    /// Get a presigned url for getting object on a given path
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use std::collections::HashMap;
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse().unwrap();
    /// let credentials = Credentials::default().unwrap();
    /// let bucket = Bucket::new(bucket_name, region, credentials).unwrap();
    ///
    /// // Add optional custom queries
    /// let mut custom_queries = HashMap::new();
    /// custom_queries.insert(
    ///    "response-content-disposition".into(),
    ///    "attachment; filename=\"test.png\"".into(),
    /// );
    ///
    /// let url = bucket.presign_get("/test.file", 86400, Some(custom_queries)).unwrap();
    /// println!("Presigned url: {}", url);
    /// ```
    pub fn presign_get<S: AsRef<str>>(
        &self,
        path: S,
        expiry_secs: u32,
        custom_queries: Option<HashMap<String, String>>,
    ) -> Result<String, S3Error> {
        validate_expiry(expiry_secs)?;
        let request = RequestImpl::new(
            self,
            path.as_ref(),
            Command::PresignGet {
                expiry_secs,
                custom_queries,
            },
        )?;
        request.presigned()
    }

    /// Get a presigned url for posting an object to a given path
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use http::HeaderMap;
    /// use http::header::HeaderName;
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse().unwrap();
    /// let credentials = Credentials::default().unwrap();
    /// let bucket = Bucket::new(bucket_name, region, credentials).unwrap();
    ///
    /// let post_policy = "eyAiZXhwaXJhdGlvbiI6ICIyMDE1LTEyLTMwVDEyOjAwOjAwLjAwMFoiLA0KICAiY29uZGl0aW9ucyI6IFsNCiAgICB7ImJ1Y2tldCI6ICJzaWd2NGV4YW1wbGVidWNrZXQifSwNCiAgICBbInN0YXJ0cy13aXRoIiwgIiRrZXkiLCAidXNlci91c2VyMS8iXSwNCiAgICB7ImFjbCI6ICJwdWJsaWMtcmVhZCJ9LA0KICAgIHsic3VjY2Vzc19hY3Rpb25fcmVkaXJlY3QiOiAiaHR0cDovL3NpZ3Y0ZXhhbXBsZWJ1Y2tldC5zMy5hbWF6b25hd3MuY29tL3N1Y2Nlc3NmdWxfdXBsb2FkLmh0bWwifSwNCiAgICBbInN0YXJ0cy13aXRoIiwgIiRDb250ZW50LVR5cGUiLCAiaW1hZ2UvIl0sDQogICAgeyJ4LWFtei1tZXRhLXV1aWQiOiAiMTQzNjUxMjM2NTEyNzQifSwNCiAgICB7IngtYW16LXNlcnZlci1zaWRlLWVuY3J5cHRpb24iOiAiQUVTMjU2In0sDQogICAgWyJzdGFydHMtd2l0aCIsICIkeC1hbXotbWV0YS10YWciLCAiIl0sDQoNCiAgICB7IngtYW16LWNyZWRlbnRpYWwiOiAiQUtJQUlPU0ZPRE5ON0VYQU1QTEUvMjAxNTEyMjkvdXMtZWFzdC0xL3MzL2F3czRfcmVxdWVzdCJ9LA0KICAgIHsieC1hbXotYWxnb3JpdGhtIjogIkFXUzQtSE1BQy1TSEEyNTYifSwNCiAgICB7IngtYW16LWRhdGUiOiAiMjAxNTEyMjlUMDAwMDAwWiIgfQ0KICBdDQp9";
    ///
    /// let url = bucket.presign_post("/test.file", 86400, post_policy.to_string()).unwrap();
    /// println!("Presigned url: {}", url);
    /// ```
    pub fn presign_post<S: AsRef<str>>(
        &self,
        path: S,
        expiry_secs: u32,
        // base64 encoded post policy document -> https://docs.aws.amazon.com/AmazonS3/latest/API/sigv4-post-example.html
        post_policy: String,
    ) -> Result<String, S3Error> {
        validate_expiry(expiry_secs)?;
        let request = RequestImpl::new(
            self,
            path.as_ref(),
            Command::PresignPost {
                expiry_secs,
                post_policy,
            },
        )?;
        request.presigned()
    }

    /// Get a presigned url for putting object to a given path
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use http::HeaderMap;
    /// use http::header::HeaderName;
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse().unwrap();
    /// let credentials = Credentials::default().unwrap();
    /// let bucket = Bucket::new(bucket_name, region, credentials).unwrap();
    ///
    /// // Add optional custom headers
    /// let mut custom_headers = HeaderMap::new();
    /// custom_headers.insert(
    ///    HeaderName::from_static("custom_header"),
    ///    "custom_value".parse().unwrap(),
    /// );
    ///
    /// let url = bucket.presign_put("/test.file", 86400, Some(custom_headers)).unwrap();
    /// println!("Presigned url: {}", url);
    /// ```
    pub fn presign_put<S: AsRef<str>>(
        &self,
        path: S,
        expiry_secs: u32,
        custom_headers: Option<HeaderMap>,
    ) -> Result<String, S3Error> {
        validate_expiry(expiry_secs)?;
        let request = RequestImpl::new(
            self,
            path.as_ref(),
            Command::PresignPut {
                expiry_secs,
                custom_headers,
            },
        )?;
        request.presigned()
    }

    /// Get a presigned url for deleting object on a given path
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse().unwrap();
    /// let credentials = Credentials::default().unwrap();
    /// let bucket = Bucket::new(bucket_name, region, credentials).unwrap();
    ///
    /// let url = bucket.presign_delete("/test.file", 86400).unwrap();
    /// println!("Presigned url: {}", url);
    /// ```
    pub fn presign_delete<S: AsRef<str>>(
        &self,
        path: S,
        expiry_secs: u32,
    ) -> Result<String, S3Error> {
        validate_expiry(expiry_secs)?;
        let request =
            RequestImpl::new(self, path.as_ref(), Command::PresignDelete { expiry_secs })?;
        request.presigned()
    }

    /// Create a new `Bucket` and instantiate it
    ///
    /// ```no_run
    /// use s3::{Bucket, BucketConfiguration};
    /// use s3::creds::Credentials;
    /// # use s3::region::Region;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let config = BucketConfiguration::default();
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let create_bucket_response = Bucket::create(bucket_name, region, credentials, config).await?;
    ///
    /// // `sync` fature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let create_bucket_response = Bucket::create(bucket_name, region, credentials, config)?;
    ///
    /// # let region: Region = "us-east-1".parse()?;
    /// # let credentials = Credentials::default()?;
    /// # let config = BucketConfiguration::default();
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let create_bucket_response = Bucket::create_blocking(bucket_name, region, credentials, config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(
        name: &str,
        region: Region,
        credentials: Credentials,
        config: BucketConfiguration,
    ) -> Result<CreateBucketResponse, S3Error> {
        let mut config = config;
        config.set_region(region.clone());
        let command = Command::CreateBucket { config };
        let bucket = Bucket::new(name, region, credentials)?;
        let request = RequestImpl::new(&bucket, "", command)?;
        let response_data = request.response_data(false).await?;
        let response_text = response_data.as_str()?;
        Ok(CreateBucketResponse {
            bucket,
            response_text: response_text.to_string(),
            response_code: response_data.status_code(),
        })
    }

    /// Create a new `Bucket` with path style and instantiate it
    ///
    /// ```no_run
    /// use s3::{Bucket, BucketConfiguration};
    /// use s3::creds::Credentials;
    /// # use s3::region::Region;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let config = BucketConfiguration::default();
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let create_bucket_response = Bucket::create_with_path_style(bucket_name, region, credentials, config).await?;
    ///
    /// // `sync` fature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let create_bucket_response = Bucket::create_with_path_style(bucket_name, region, credentials, config)?;
    ///
    /// # let region: Region = "us-east-1".parse()?;
    /// # let credentials = Credentials::default()?;
    /// # let config = BucketConfiguration::default();
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let create_bucket_response = Bucket::create_with_path_style_blocking(bucket_name, region, credentials, config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_with_path_style(
        name: &str,
        region: Region,
        credentials: Credentials,
        config: BucketConfiguration,
    ) -> Result<CreateBucketResponse, S3Error> {
        let mut config = config;
        config.set_region(region.clone());
        let command = Command::CreateBucket { config };
        let bucket = Bucket::new(name, region, credentials)?.with_path_style();
        let request = RequestImpl::new(&bucket, "", command)?;
        let response_data = request.response_data(false).await?;
        let response_text = response_data.to_string()?;
        Ok(CreateBucketResponse {
            bucket,
            response_text,
            response_code: response_data.status_code(),
        })
    }

    /// Delete existing `Bucket`
    ///
    /// # Example
    /// ```rust,no_run
    /// use s3::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse().unwrap();
    /// let credentials = Credentials::default().unwrap();
    /// let bucket = Bucket::new(bucket_name, region, credentials).unwrap();
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// bucket.delete().await.unwrap();
    /// // `sync` fature will produce an identical method
    ///
    /// #[cfg(feature = "sync")]
    /// bucket.delete().unwrap();
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    ///
    /// #[cfg(feature = "blocking")]
    /// bucket.delete_blocking().unwrap();
    ///
    /// # Ok(())
    /// # }
    /// ```

    pub async fn delete(&self) -> Result<u16, S3Error> {
        let command = Command::DeleteBucket;
        let request = RequestImpl::new(self, "", command)?;
        let response_data = request.response_data(false).await?;
        Ok(response_data.status_code())
    }

    /// Instantiate an existing `Bucket`.
    ///
    /// # Example
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    ///
    /// // Fake  credentials so we don't access user's real credentials in tests
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse().unwrap();
    /// let credentials = Credentials::default().unwrap();
    ///
    /// let bucket = Bucket::new(bucket_name, region, credentials).unwrap();
    /// ```
    pub fn new(name: &str, region: Region, credentials: Credentials) -> Result<Bucket, S3Error> {
        Ok(Bucket {
            name: name.into(),
            region,
            credentials: Arc::new(RwLock::new(credentials)),
            extra_headers: HeaderMap::new(),
            extra_query: HashMap::new(),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            path_style: false,
            listobjects_v2: true,
        })
    }

    /// Instantiate a public existing `Bucket`.
    ///
    /// # Example
    /// ```no_run
    /// use s3::bucket::Bucket;
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse().unwrap();
    ///
    /// let bucket = Bucket::new_public(bucket_name, region).unwrap();
    /// ```
    pub fn new_public(name: &str, region: Region) -> Result<Bucket, S3Error> {
        Ok(Bucket {
            name: name.into(),
            region,
            credentials: Arc::new(RwLock::new(Credentials::anonymous()?)),
            extra_headers: HeaderMap::new(),
            extra_query: HashMap::new(),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            path_style: false,
            listobjects_v2: true,
        })
    }

    pub fn with_path_style(&self) -> Bucket {
        Bucket {
            name: self.name.clone(),
            region: self.region.clone(),
            credentials: self.credentials.clone(),
            extra_headers: self.extra_headers.clone(),
            extra_query: self.extra_query.clone(),
            request_timeout: self.request_timeout,
            path_style: true,
            listobjects_v2: self.listobjects_v2,
        }
    }

    pub fn with_extra_headers(&self, extra_headers: HeaderMap) -> Bucket {
        Bucket {
            name: self.name.clone(),
            region: self.region.clone(),
            credentials: self.credentials.clone(),
            extra_headers,
            extra_query: self.extra_query.clone(),
            request_timeout: self.request_timeout,
            path_style: self.path_style,
            listobjects_v2: self.listobjects_v2,
        }
    }

    pub fn with_extra_query(&self, extra_query: HashMap<String, String>) -> Bucket {
        Bucket {
            name: self.name.clone(),
            region: self.region.clone(),
            credentials: self.credentials.clone(),
            extra_headers: self.extra_headers.clone(),
            extra_query,
            request_timeout: self.request_timeout,
            path_style: self.path_style,
            listobjects_v2: self.listobjects_v2,
        }
    }

    pub fn with_request_timeout(&self, request_timeout: Duration) -> Bucket {
        Bucket {
            name: self.name.clone(),
            region: self.region.clone(),
            credentials: self.credentials.clone(),
            extra_headers: self.extra_headers.clone(),
            extra_query: self.extra_query.clone(),
            request_timeout: Some(request_timeout),
            path_style: self.path_style,
            listobjects_v2: self.listobjects_v2,
        }
    }

    pub fn with_listobjects_v1(&self) -> Bucket {
        Bucket {
            name: self.name.clone(),
            region: self.region.clone(),
            credentials: self.credentials.clone(),
            extra_headers: self.extra_headers.clone(),
            extra_query: self.extra_query.clone(),
            request_timeout: self.request_timeout,
            path_style: self.path_style,
            listobjects_v2: false,
        }
    }

    /// Copy file from an S3 path, internally within the same bucket.
    ///
    /// # Example:
    ///
    /// ```rust,no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let code = bucket.copy_object_internal("/from.file", "/to.file").await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let code = bucket.copy_object_internal("/from.file", "/to.file")?;
    ///
    /// # Ok(())
    /// # }
    /// ```

    pub async fn copy_object_internal<F: AsRef<str>, T: AsRef<str>>(
        &self,
        from: F,
        to: T,
    ) -> Result<u16, S3Error> {
        let fq_from = {
            let from = from.as_ref();
            let from = from.strip_prefix('/').unwrap_or(from);
            format!("{bucket}/{path}", bucket = self.name(), path = from)
        };
        self.copy_object(fq_from, to).await
    }

    async fn copy_object<F: AsRef<str>, T: AsRef<str>>(
        &self,
        from: F,
        to: T,
    ) -> Result<u16, S3Error> {
        let command = Command::CopyObject {
            from: from.as_ref(),
        };
        let request = RequestImpl::new(self, to.as_ref(), command)?;
        let response_data = request.response_data(false).await?;
        Ok(response_data.status_code())
    }

    /// Gets file from an S3 path.
    ///
    /// # Example:
    ///
    /// ```rust,no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let response_data = bucket.get_object("/test.file").await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let response_data = bucket.get_object("/test.file")?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let response_data = bucket.get_object_blocking("/test.file")?;
    /// # Ok(())
    /// # }
    /// ```

    pub async fn get_object<S: AsRef<str>>(&self, path: S) -> Result<ResponseData, S3Error> {
        let command = Command::GetObject;
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        request.response_data(false).await
    }

    pub async fn put_bucket_cors(
        &self,
        cors_config: CorsConfiguration,
    ) -> Result<ResponseData, S3Error> {
        let command = Command::PutBucketCors {
            configuration: cors_config,
        };
        let request = RequestImpl::new(self, "?cors", command)?;
        request.response_data(false).await
    }

    /// Gets torrent from an S3 path.
    ///
    /// # Example:
    ///
    /// ```rust,no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let response_data = bucket.get_object_torrent("/test.file").await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let response_data = bucket.get_object_torrent("/test.file")?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let response_data = bucket.get_object_torrent_blocking("/test.file")?;
    /// # Ok(())
    /// # }
    /// ```

    pub async fn get_object_torrent<S: AsRef<str>>(
        &self,
        path: S,
    ) -> Result<ResponseData, S3Error> {
        let command = Command::GetObjectTorrent;
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        request.response_data(false).await
    }

    /// Gets specified inclusive byte range of file from an S3 path.
    ///
    /// # Example:
    ///
    /// ```rust,no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let response_data = bucket.get_object_range("/test.file", 0, Some(31)).await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let response_data = bucket.get_object_range("/test.file", 0, Some(31))?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let response_data = bucket.get_object_range_blocking("/test.file", 0, Some(31))?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn get_object_range<S: AsRef<str>>(
        &self,
        path: S,
        start: u64,
        end: Option<u64>,
    ) -> Result<ResponseData, S3Error> {
        if let Some(end) = end {
            assert!(start < end);
        }

        let command = Command::GetObjectRange { start, end };
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        request.response_data(false).await
    }

    /// Stream range of bytes from S3 path to a local file, generic over T: Write.
    ///
    /// # Example:
    ///
    /// ```rust,no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    /// use std::fs::File;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    /// let mut output_file = File::create("output_file").expect("Unable to create file");
    /// let mut async_output_file = tokio::fs::File::create("async_output_file").await.expect("Unable to create file");
    /// #[cfg(feature = "with-async-std")]
    /// let mut async_output_file = async_std::fs::File::create("async_output_file").await.expect("Unable to create file");
    ///
    /// let start = 0;
    /// let end = Some(1024);
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let status_code = bucket.get_object_range_to_writer("/test.file", start, end, &mut async_output_file).await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let status_code = bucket.get_object_range_to_writer("/test.file", start, end, &mut output_file)?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features. Based of the async branch
    /// #[cfg(feature = "blocking")]
    /// let status_code = bucket.get_object_range_to_writer_blocking("/test.file", start, end, &mut async_output_file)?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_object_range_to_writer<T: AsyncWrite + Send + Unpin, S: AsRef<str>>(
        &self,
        path: S,
        start: u64,
        end: Option<u64>,
        writer: &mut T,
    ) -> Result<u16, S3Error> {
        if let Some(end) = end {
            assert!(start < end);
        }

        let command = Command::GetObjectRange { start, end };
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        request.response_data_to_writer(writer).await
    }

    /// Stream file from S3 path to a local file, generic over T: Write.
    ///
    /// # Example:
    ///
    /// ```rust,no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    /// use std::fs::File;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    /// let mut output_file = File::create("output_file").expect("Unable to create file");
    /// let mut async_output_file = tokio::fs::File::create("async_output_file").await.expect("Unable to create file");
    /// #[cfg(feature = "with-async-std")]
    /// let mut async_output_file = async_std::fs::File::create("async_output_file").await.expect("Unable to create file");
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let status_code = bucket.get_object_to_writer("/test.file", &mut async_output_file).await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let status_code = bucket.get_object_to_writer("/test.file", &mut output_file)?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features. Based of the async branch
    /// #[cfg(feature = "blocking")]
    /// let status_code = bucket.get_object_to_writer_blocking("/test.file", &mut async_output_file)?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn get_object_to_writer<T: AsyncWrite + Send + Unpin, S: AsRef<str>>(
        &self,
        path: S,
        writer: &mut T,
    ) -> Result<u16, S3Error> {
        let command = Command::GetObject;
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        request.response_data_to_writer(writer).await
    }

    /// Stream file from S3 path to a local file using an async stream.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    /// #[cfg(feature = "with-tokio")]
    /// use tokio_stream::StreamExt;
    /// #[cfg(feature = "with-tokio")]
    /// use tokio::io::AsyncWriteExt;
    /// #[cfg(feature = "with-async-std")]
    /// use futures_util::StreamExt;
    /// #[cfg(feature = "with-async-std")]
    /// use futures_util::AsyncWriteExt;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    /// let path = "path";
    ///
    /// let mut response_data_stream = bucket.get_object_stream(path).await?;
    ///
    /// #[cfg(feature = "with-tokio")]
    /// let mut async_output_file = tokio::fs::File::create("async_output_file").await.expect("Unable to create file");
    /// #[cfg(feature = "with-async-std")]
    /// let mut async_output_file = async_std::fs::File::create("async_output_file").await.expect("Unable to create file");
    ///
    /// while let Some(chunk) = response_data_stream.bytes().next().await {
    ///     async_output_file.write_all(&chunk.unwrap()).await?;
    /// }
    ///
    /// #
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(any(feature = "with-tokio", feature = "with-async-std"))]
    pub async fn get_object_stream<S: AsRef<str>>(
        &self,
        path: S,
    ) -> Result<ResponseDataStream, S3Error> {
        let command = Command::GetObject;
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        request.response_data_to_stream().await
    }

    /// Stream file from local path to s3, generic over T: Write.
    ///
    /// # Example:
    ///
    /// ```rust,no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    /// use std::fs::File;
    /// use std::io::Write;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    /// let path = "path";
    /// let test: Vec<u8> = (0..1000).map(|_| 42).collect();
    /// let mut file = File::create(path)?;
    /// // tokio open file
    /// let mut async_output_file = tokio::fs::File::create("async_output_file").await.expect("Unable to create file");
    /// file.write_all(&test)?;
    ///
    /// // Generic over std::io::Read
    /// #[cfg(feature = "with-tokio")]
    /// let status_code = bucket.put_object_stream(&mut async_output_file, "/path").await?;
    ///
    ///
    /// #[cfg(feature = "with-async-std")]
    /// let mut async_output_file = async_std::fs::File::create("async_output_file").await.expect("Unable to create file");
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// // Generic over std::io::Read
    /// let status_code = bucket.put_object_stream(&mut path, "/path")?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let status_code = bucket.put_object_stream_blocking(&mut path, "/path")?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn put_object_stream<R: AsyncRead + Unpin>(
        &self,
        reader: &mut R,
        s3_path: impl AsRef<str>,
    ) -> Result<PutStreamResponse, S3Error> {
        self._put_object_stream_with_content_type(
            reader,
            s3_path.as_ref(),
            "application/octet-stream",
        )
        .await
    }

    /// Stream file from local path to s3, generic over T: Write with explicit content type.
    ///
    /// # Example:
    ///
    /// ```rust,no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    /// use std::fs::File;
    /// use std::io::Write;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    /// let path = "path";
    /// let test: Vec<u8> = (0..1000).map(|_| 42).collect();
    /// let mut file = File::create(path)?;
    /// file.write_all(&test)?;
    ///
    /// #[cfg(feature = "with-tokio")]
    /// let mut async_output_file = tokio::fs::File::create("async_output_file").await.expect("Unable to create file");
    ///
    /// #[cfg(feature = "with-async-std")]
    /// let mut async_output_file = async_std::fs::File::create("async_output_file").await.expect("Unable to create file");
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// // Generic over std::io::Read
    /// let status_code = bucket
    ///     .put_object_stream_with_content_type(&mut async_output_file, "/path", "application/octet-stream")
    ///     .await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// // Generic over std::io::Read
    /// let status_code = bucket
    ///     .put_object_stream_with_content_type(&mut path, "/path", "application/octet-stream")?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let status_code = bucket
    ///     .put_object_stream_with_content_type_blocking(&mut path, "/path", "application/octet-stream")?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn put_object_stream_with_content_type<R: AsyncRead + Unpin>(
        &self,
        reader: &mut R,
        s3_path: impl AsRef<str>,
        content_type: impl AsRef<str>,
    ) -> Result<PutStreamResponse, S3Error> {
        self._put_object_stream_with_content_type(reader, s3_path.as_ref(), content_type.as_ref())
            .await
    }

    async fn make_multipart_request(
        &self,
        path: &str,
        chunk: Vec<u8>,
        part_number: u32,
        upload_id: &str,
        content_type: &str,
    ) -> Result<ResponseData, S3Error> {
        let command = Command::PutObject {
            content: &chunk,
            multipart: Some(Multipart::new(part_number, upload_id)), // upload_id: &msg.upload_id,
            content_type,
        };
        let request = RequestImpl::new(self, path, command)?;
        request.response_data(true).await
    }

    async fn _put_object_stream_with_content_type<R: AsyncRead + Unpin>(
        &self,
        reader: &mut R,
        s3_path: &str,
        content_type: &str,
    ) -> Result<PutStreamResponse, S3Error> {
        // If the file is smaller CHUNK_SIZE, just do a regular upload.
        // Otherwise perform a multi-part upload.
        let first_chunk = crate::utils::read_chunk_async(reader).await?;
        if first_chunk.len() < CHUNK_SIZE {
            let total_size = first_chunk.len();
            let response_data = self
                .put_object_with_content_type(s3_path, first_chunk.as_slice(), content_type)
                .await?;
            if response_data.status_code() >= 300 {
                return Err(error_from_response_data(response_data)?);
            }
            return Ok(PutStreamResponse::new(
                response_data.status_code(),
                total_size,
            ));
        }

        let msg = self
            .initiate_multipart_upload(s3_path, content_type)
            .await?;
        let path = msg.key;
        let upload_id = &msg.upload_id;

        let mut part_number: u32 = 0;
        let mut etags = Vec::new();

        // Collect request handles
        let mut handles = vec![];
        let mut total_size = 0;
        loop {
            let chunk = if part_number == 0 {
                first_chunk.clone()
            } else {
                crate::utils::read_chunk_async(reader).await?
            };
            total_size += chunk.len();

            let done = chunk.len() < CHUNK_SIZE;

            // Start chunk upload
            part_number += 1;
            handles.push(self.make_multipart_request(
                &path,
                chunk,
                part_number,
                upload_id,
                content_type,
            ));

            if done {
                break;
            }
        }

        // Wait for all chunks to finish (or fail)
        let responses = futures::future::join_all(handles).await;

        for response in responses {
            let response_data = response?;
            if !(200..300).contains(&response_data.status_code()) {
                // if chunk upload failed - abort the upload
                match self.abort_upload(&path, upload_id).await {
                    Ok(_) => {
                        return Err(error_from_response_data(response_data)?);
                    }
                    Err(error) => {
                        return Err(error);
                    }
                }
            }

            let etag = response_data.as_str()?;
            etags.push(etag.to_string());
        }

        // Finish the upload
        let inner_data = etags
            .clone()
            .into_iter()
            .enumerate()
            .map(|(i, x)| Part {
                etag: x,
                part_number: i as u32 + 1,
            })
            .collect::<Vec<Part>>();
        let response_data = self
            .complete_multipart_upload(&path, &msg.upload_id, inner_data)
            .await?;

        Ok(PutStreamResponse::new(
            response_data.status_code(),
            total_size,
        ))
    }

    /// Initiate multipart upload to s3.

    pub async fn initiate_multipart_upload(
        &self,
        s3_path: &str,
        content_type: &str,
    ) -> Result<InitiateMultipartUploadResponse, S3Error> {
        let command = Command::InitiateMultipartUpload { content_type };
        let request = RequestImpl::new(self, s3_path, command)?;
        let response_data = request.response_data(false).await?;
        if response_data.status_code() >= 300 {
            return Err(error_from_response_data(response_data)?);
        }

        let msg: InitiateMultipartUploadResponse =
            quick_xml::de::from_str(response_data.as_str()?)?;
        Ok(msg)
    }

    /// Upload a streamed multipart chunk to s3 using a previously initiated multipart upload

    pub async fn put_multipart_stream<R: Read + Unpin>(
        &self,
        reader: &mut R,
        path: &str,
        part_number: u32,
        upload_id: &str,
        content_type: &str,
    ) -> Result<Part, S3Error> {
        let chunk = crate::utils::read_chunk(reader)?;
        self.put_multipart_chunk(chunk, path, part_number, upload_id, content_type)
            .await
    }

    /// Upload a buffered multipart chunk to s3 using a previously initiated multipart upload
    pub async fn put_multipart_chunk(
        &self,
        chunk: Vec<u8>,
        path: &str,
        part_number: u32,
        upload_id: &str,
        content_type: &str,
    ) -> Result<Part, S3Error> {
        let command = Command::PutObject {
            // part_number,
            content: &chunk,
            multipart: Some(Multipart::new(part_number, upload_id)), // upload_id: &msg.upload_id,
            content_type,
        };
        let request = RequestImpl::new(self, path, command)?;
        let response_data = request.response_data(true).await?;
        if !(200..300).contains(&response_data.status_code()) {
            // if chunk upload failed - abort the upload
            match self.abort_upload(path, upload_id).await {
                Ok(_) => {
                    return Err(error_from_response_data(response_data)?);
                }
                Err(error) => {
                    return Err(error);
                }
            }
        }
        let etag = response_data.as_str()?;
        Ok(Part {
            etag: etag.to_string(),
            part_number,
        })
    }

    /// Completes a previously initiated multipart upload, with optional final data chunks
    pub async fn complete_multipart_upload(
        &self,
        path: &str,
        upload_id: &str,
        parts: Vec<Part>,
    ) -> Result<ResponseData, S3Error> {
        let data = CompleteMultipartUploadData { parts };
        let complete = Command::CompleteMultipartUpload { upload_id, data };
        let complete_request = RequestImpl::new(self, path, complete)?;
        complete_request.response_data(false).await
    }

    /// Get Bucket location.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let (region, status_code) = bucket.location().await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let (region, status_code) = bucket.location()?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let (region, status_code) = bucket.location_blocking()?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn location(&self) -> Result<(Region, u16), S3Error> {
        let request = RequestImpl::new(self, "?location", Command::GetBucketLocation)?;
        let response_data = request.response_data(false).await?;
        let region_string = String::from_utf8_lossy(response_data.as_slice());
        let region = match quick_xml::de::from_reader(region_string.as_bytes()) {
            Ok(r) => {
                let location_result: BucketLocationResult = r;
                location_result.region.parse()?
            }
            Err(e) => {
                if response_data.status_code() == 200 {
                    Region::Custom {
                        region: "Custom".to_string(),
                        endpoint: "".to_string(),
                    }
                } else {
                    Region::Custom {
                        region: format!("Error encountered : {}", e),
                        endpoint: "".to_string(),
                    }
                }
            }
        };
        Ok((region, response_data.status_code()))
    }

    /// Delete file from an S3 path.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let response_data = bucket.delete_object("/test.file").await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let response_data = bucket.delete_object("/test.file")?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let response_data = bucket.delete_object_blocking("/test.file")?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_object<S: AsRef<str>>(&self, path: S) -> Result<ResponseData, S3Error> {
        let command = Command::DeleteObject;
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        request.response_data(false).await
    }

    /// Head object from S3.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let (head_object_result, code) = bucket.head_object("/test.png").await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let (head_object_result, code) = bucket.head_object("/test.png")?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let (head_object_result, code) = bucket.head_object_blocking("/test.png")?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub async fn head_object<S: AsRef<str>>(
        &self,
        path: S,
    ) -> Result<(HeadObjectResult, u16), S3Error> {
        let command = Command::HeadObject;
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        let (headers, status) = request.response_header().await?;
        let header_object = HeadObjectResult::from(&headers);
        Ok((header_object, status))
    }

    /// Put into an S3 bucket, with explicit content-type.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    /// let content = "I want to go to S3".as_bytes();
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let response_data = bucket.put_object_with_content_type("/test.file", content, "text/plain").await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let response_data = bucket.put_object_with_content_type("/test.file", content, "text/plain")?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let response_data = bucket.put_object_with_content_type_blocking("/test.file", content, "text/plain")?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn put_object_with_content_type<S: AsRef<str>>(
        &self,
        path: S,
        content: &[u8],
        content_type: &str,
    ) -> Result<ResponseData, S3Error> {
        let command = Command::PutObject {
            content,
            content_type,
            multipart: None,
        };
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        request.response_data(true).await
    }

    /// Put into an S3 bucket.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    /// let content = "I want to go to S3".as_bytes();
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let response_data = bucket.put_object("/test.file", content).await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let response_data = bucket.put_object("/test.file", content)?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let response_data = bucket.put_object_blocking("/test.file", content)?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn put_object<S: AsRef<str>>(
        &self,
        path: S,
        content: &[u8],
    ) -> Result<ResponseData, S3Error> {
        self.put_object_with_content_type(path, content, "application/octet-stream")
            .await
    }

    fn _tags_xml<S: AsRef<str>>(&self, tags: &[(S, S)]) -> String {
        let mut s = String::new();
        let content = tags
            .iter()
            .map(|(name, value)| {
                format!(
                    "<Tag><Key>{}</Key><Value>{}</Value></Tag>",
                    name.as_ref(),
                    value.as_ref()
                )
            })
            .fold(String::new(), |mut a, b| {
                a.push_str(b.as_str());
                a
            });
        s.push_str("<Tagging><TagSet>");
        s.push_str(&content);
        s.push_str("</TagSet></Tagging>");
        s
    }

    /// Tag an S3 object.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let response_data = bucket.put_object_tagging("/test.file", &[("Tag1", "Value1"), ("Tag2", "Value2")]).await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let response_data = bucket.put_object_tagging("/test.file", &[("Tag1", "Value1"), ("Tag2", "Value2")])?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let response_data = bucket.put_object_tagging_blocking("/test.file", &[("Tag1", "Value1"), ("Tag2", "Value2")])?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn put_object_tagging<S: AsRef<str>>(
        &self,
        path: &str,
        tags: &[(S, S)],
    ) -> Result<ResponseData, S3Error> {
        let content = self._tags_xml(tags);
        let command = Command::PutObjectTagging { tags: &content };
        let request = RequestImpl::new(self, path, command)?;
        request.response_data(false).await
    }

    /// Delete tags from an S3 object.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let response_data = bucket.delete_object_tagging("/test.file").await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let response_data = bucket.delete_object_tagging("/test.file")?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let response_data = bucket.delete_object_tagging_blocking("/test.file")?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn delete_object_tagging<S: AsRef<str>>(
        &self,
        path: S,
    ) -> Result<ResponseData, S3Error> {
        let command = Command::DeleteObjectTagging;
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        request.response_data(false).await
    }

    /// Retrieve an S3 object list of tags.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let response_data = bucket.get_object_tagging("/test.file").await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let response_data = bucket.get_object_tagging("/test.file")?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let response_data = bucket.get_object_tagging_blocking("/test.file")?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "tags")]

    pub async fn get_object_tagging<S: AsRef<str>>(
        &self,
        path: S,
    ) -> Result<(Vec<Tag>, u16), S3Error> {
        let command = Command::GetObjectTagging {};
        let request = RequestImpl::new(self, path.as_ref(), command)?;
        let result = request.response_data(false).await?;

        let mut tags = Vec::new();

        if result.status_code() == 200 {
            let result_string = String::from_utf8_lossy(result.as_slice());

            // Add namespace if it doesn't exist
            let ns = "http://s3.amazonaws.com/doc/2006-03-01/";
            let result_string =
                if let Err(minidom::Error::MissingNamespace) = result_string.parse::<Element>() {
                    result_string
                        .replace("<Tagging>", &format!("<Tagging xmlns=\"{}\">", ns))
                        .into()
                } else {
                    result_string
                };

            if let Ok(tagging) = result_string.parse::<Element>() {
                for tag_set in tagging.children() {
                    if tag_set.is("TagSet", ns) {
                        for tag in tag_set.children() {
                            if tag.is("Tag", ns) {
                                let key = if let Some(element) = tag.get_child("Key", ns) {
                                    element.text()
                                } else {
                                    "Could not parse Key from Tag".to_string()
                                };
                                let value = if let Some(element) = tag.get_child("Value", ns) {
                                    element.text()
                                } else {
                                    "Could not parse Values from Tag".to_string()
                                };
                                tags.push(Tag { key, value });
                            }
                        }
                    }
                }
            }
        }

        Ok((tags, result.status_code()))
    }

    pub async fn list_page(
        &self,
        prefix: String,
        delimiter: Option<String>,
        continuation_token: Option<String>,
        start_after: Option<String>,
        max_keys: Option<usize>,
    ) -> Result<(ListBucketResult, u16), S3Error> {
        let command = if self.listobjects_v2 {
            Command::ListObjectsV2 {
                prefix,
                delimiter,
                continuation_token,
                start_after,
                max_keys,
            }
        } else {
            // In the v1 ListObjects request, there is only one "marker"
            // field that serves as both the initial starting position,
            // and as the continuation token.
            Command::ListObjects {
                prefix,
                delimiter,
                marker: std::cmp::max(continuation_token, start_after),
                max_keys,
            }
        };
        let request = RequestImpl::new(self, "/", command)?;
        let response_data = request.response_data(false).await?;
        let list_bucket_result = quick_xml::de::from_reader(response_data.as_slice())?;

        Ok((list_bucket_result, response_data.status_code()))
    }

    /// List the contents of an S3 bucket.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let results = bucket.list("/".to_string(), Some("/".to_string())).await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let results = bucket.list("/".to_string(), Some("/".to_string()))?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let results = bucket.list_blocking("/".to_string(), Some("/".to_string()))?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn list(
        &self,
        prefix: String,
        delimiter: Option<String>,
    ) -> Result<Vec<ListBucketResult>, S3Error> {
        let the_bucket = self.to_owned();
        let mut results = Vec::new();
        let mut continuation_token = None;

        loop {
            let (list_bucket_result, _) = the_bucket
                .list_page(
                    prefix.clone(),
                    delimiter.clone(),
                    continuation_token,
                    None,
                    None,
                )
                .await?;
            continuation_token = list_bucket_result.next_continuation_token.clone();
            results.push(list_bucket_result);
            if continuation_token.is_none() {
                break;
            }
        }

        Ok(results)
    }

    pub async fn list_multiparts_uploads_page(
        &self,
        prefix: Option<&str>,
        delimiter: Option<&str>,
        key_marker: Option<String>,
        max_uploads: Option<usize>,
    ) -> Result<(ListMultipartUploadsResult, u16), S3Error> {
        let command = Command::ListMultipartUploads {
            prefix,
            delimiter,
            key_marker,
            max_uploads,
        };
        let request = RequestImpl::new(self, "/", command)?;
        let response_data = request.response_data(false).await?;
        let list_bucket_result = quick_xml::de::from_reader(response_data.as_slice())?;

        Ok((list_bucket_result, response_data.status_code()))
    }

    /// List the ongoing multipart uploads of an S3 bucket. This may be useful to cleanup failed
    /// uploads, together with [`crate::bucket::Bucket::abort_upload`].
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let results = bucket.list_multiparts_uploads(Some("/"), Some("/")).await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let results = bucket.list_multiparts_uploads(Some("/"), Some("/"))?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let results = bucket.list_multiparts_uploads_blocking(Some("/"), Some("/"))?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn list_multiparts_uploads(
        &self,
        prefix: Option<&str>,
        delimiter: Option<&str>,
    ) -> Result<Vec<ListMultipartUploadsResult>, S3Error> {
        let the_bucket = self.to_owned();
        let mut results = Vec::new();
        let mut next_marker: Option<String> = None;

        loop {
            let (list_multiparts_uploads_result, _) = the_bucket
                .list_multiparts_uploads_page(prefix, delimiter, next_marker, None)
                .await?;

            let is_truncated = list_multiparts_uploads_result.is_truncated;
            next_marker = list_multiparts_uploads_result.next_marker.clone();
            results.push(list_multiparts_uploads_result);

            if !is_truncated {
                break;
            }
        }

        Ok(results)
    }

    /// Abort a running multipart upload.
    ///
    /// # Example:
    ///
    /// ```no_run
    /// use s3::bucket::Bucket;
    /// use s3::creds::Credentials;
    /// use anyhow::Result;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    ///
    /// let bucket_name = "rust-s3-test";
    /// let region = "us-east-1".parse()?;
    /// let credentials = Credentials::default()?;
    /// let bucket = Bucket::new(bucket_name, region, credentials)?;
    ///
    /// // Async variant with `tokio` or `async-std` features
    /// let results = bucket.abort_upload("/some/file.txt", "ZDFjM2I0YmEtMzU3ZC00OTQ1LTlkNGUtMTgxZThjYzIwNjA2").await?;
    ///
    /// // `sync` feature will produce an identical method
    /// #[cfg(feature = "sync")]
    /// let results = bucket.abort_upload("/some/file.txt", "ZDFjM2I0YmEtMzU3ZC00OTQ1LTlkNGUtMTgxZThjYzIwNjA2")?;
    ///
    /// // Blocking variant, generated with `blocking` feature in combination
    /// // with `tokio` or `async-std` features.
    /// #[cfg(feature = "blocking")]
    /// let results = bucket.abort_upload_blocking("/some/file.txt", "ZDFjM2I0YmEtMzU3ZC00OTQ1LTlkNGUtMTgxZThjYzIwNjA2")?;
    /// #
    /// # Ok(())
    /// # }
    /// ```

    pub async fn abort_upload(&self, key: &str, upload_id: &str) -> Result<(), S3Error> {
        let abort = Command::AbortMultipartUpload { upload_id };
        let abort_request = RequestImpl::new(self, key, abort)?;
        let response_data = abort_request.response_data(false).await?;

        if (200..300).contains(&response_data.status_code()) {
            Ok(())
        } else {
            let utf8_content = String::from_utf8(response_data.as_slice().to_vec())?;
            Err(S3Error::HttpFailWithBody(
                response_data.status_code(),
                utf8_content,
            ))
        }
    }

    /// Get path_style field of the Bucket struct
    pub fn is_path_style(&self) -> bool {
        self.path_style
    }

    /// Get negated path_style field of the Bucket struct
    pub fn is_subdomain_style(&self) -> bool {
        !self.path_style
    }

    /// Configure bucket to use path-style urls and headers
    pub fn set_path_style(&mut self) {
        self.path_style = true;
    }

    /// Configure bucket to use subdomain style urls and headers \[default\]
    pub fn set_subdomain_style(&mut self) {
        self.path_style = false;
    }

    /// Configure bucket to apply this request timeout to all HTTP
    /// requests, or no (infinity) timeout if `None`.  Defaults to
    /// 30 seconds.
    ///
    /// Only the [`attohttpc`] and the [`hyper`] backends obey this option;
    /// async code may instead await with a timeout.
    pub fn set_request_timeout(&mut self, timeout: Option<Duration>) {
        self.request_timeout = timeout;
    }

    /// Configure bucket to use the older ListObjects API
    ///
    /// If your provider doesn't support the ListObjectsV2 interface, set this to
    /// use the v1 ListObjects interface instead. This is currently needed at least
    /// for Google Cloud Storage.
    pub fn set_listobjects_v1(&mut self) {
        self.listobjects_v2 = false;
    }

    /// Configure bucket to use the newer ListObjectsV2 API
    pub fn set_listobjects_v2(&mut self) {
        self.listobjects_v2 = true;
    }

    /// Get a reference to the name of the S3 bucket.
    pub fn name(&self) -> String {
        self.name.to_string()
    }

    // Get a reference to the hostname of the S3 API endpoint.
    pub fn host(&self) -> String {
        if self.path_style {
            self.path_style_host()
        } else {
            self.subdomain_style_host()
        }
    }

    pub fn url(&self) -> String {
        if self.path_style {
            format!(
                "{}://{}/{}",
                self.scheme(),
                self.path_style_host(),
                self.name()
            )
        } else {
            format!("{}://{}", self.scheme(), self.subdomain_style_host())
        }
    }

    /// Get a paths-style reference to the hostname of the S3 API endpoint.
    pub fn path_style_host(&self) -> String {
        self.region.host()
    }

    pub fn subdomain_style_host(&self) -> String {
        format!("{}.{}", self.name, self.region.host())
    }

    // pub fn self_host(&self) -> String {
    //     format!("{}.{}", self.name, self.region.host())
    // }

    pub fn scheme(&self) -> String {
        self.region.scheme()
    }

    /// Get the region this object will connect to.
    pub fn region(&self) -> Region {
        self.region.clone()
    }

    /// Get a reference to the AWS access key.
    pub fn access_key(&self) -> Result<Option<String>, S3Error> {
        Ok(self
            .credentials()
            .try_read()
            .map_err(|_| S3Error::RLCredentials)?
            .access_key
            .clone()
            .map(|key| key.replace('\n', "")))
    }

    /// Get a reference to the AWS secret key.
    pub fn secret_key(&self) -> Result<Option<String>, S3Error> {
        Ok(self
            .credentials()
            .try_read()
            .map_err(|_| S3Error::RLCredentials)?
            .secret_key
            .clone()
            .map(|key| key.replace('\n', "")))
    }

    /// Get a reference to the AWS security token.
    pub fn security_token(&self) -> Result<Option<String>, S3Error> {
        Ok(self
            .credentials()
            .try_read()
            .map_err(|_| S3Error::RLCredentials)?
            .security_token
            .clone())
    }

    /// Get a reference to the AWS session token.
    pub fn session_token(&self) -> Result<Option<String>, S3Error> {
        Ok(self
            .credentials()
            .try_read()
            .map_err(|_| S3Error::RLCredentials)?
            .session_token
            .clone())
    }

    /// Get a reference to the full [`Credentials`](struct.Credentials.html)
    /// object used by this `Bucket`.
    pub fn credentials(&self) -> Arc<RwLock<Credentials>> {
        self.credentials.clone()
    }

    /// Change the credentials used by the Bucket.
    pub fn set_credentials(&mut self, credentials: Credentials) {
        self.credentials = Arc::new(RwLock::new(credentials));
    }

    /// Add an extra header to send with requests to S3.
    ///
    /// Add an extra header to send with requests. Note that the library
    /// already sets a number of headers - headers set with this method will be
    /// overridden by the library headers:
    ///   * Host
    ///   * Content-Type
    ///   * Date
    ///   * Content-Length
    ///   * Authorization
    ///   * X-Amz-Content-Sha256
    ///   * X-Amz-Date
    pub fn add_header(&mut self, key: &str, value: &str) {
        self.extra_headers
            .insert(HeaderName::from_str(key).unwrap(), value.parse().unwrap());
    }

    /// Get a reference to the extra headers to be passed to the S3 API.
    pub fn extra_headers(&self) -> &HeaderMap {
        &self.extra_headers
    }

    /// Get a mutable reference to the extra headers to be passed to the S3
    /// API.
    pub fn extra_headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.extra_headers
    }

    /// Add an extra query pair to the URL used for S3 API access.
    pub fn add_query(&mut self, key: &str, value: &str) {
        self.extra_query.insert(key.into(), value.into());
    }

    /// Get a reference to the extra query pairs to be passed to the S3 API.
    pub fn extra_query(&self) -> &Query {
        &self.extra_query
    }

    /// Get a mutable reference to the extra query pairs to be passed to the S3
    /// API.
    pub fn extra_query_mut(&mut self) -> &mut Query {
        &mut self.extra_query
    }

    pub fn request_timeout(&self) -> Option<Duration> {
        self.request_timeout
    }
}
