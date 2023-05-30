use std::collections::HashMap;

use bytes::Bytes;

#[derive(Debug)]
pub struct ResponseData {
    bytes: Bytes,
    status_code: u16,
    headers: HashMap<String, String>,
}

pub type DataStream = Pin<Box<dyn Stream<Item = StreamItem> + Send>>;
pub type StreamItem = Result<Bytes, S3Error>;

pub struct ResponseDataStream {
    pub bytes: DataStream,
    pub status_code: u16,
}

impl ResponseDataStream {
    pub fn bytes(&mut self) -> &mut DataStream {
        &mut self.bytes
    }
}

impl From<ResponseData> for Vec<u8> {
    fn from(data: ResponseData) -> Vec<u8> {
        data.to_vec()
    }
}

impl ResponseData {
    pub fn new(bytes: Bytes, status_code: u16, headers: HashMap<String, String>) -> ResponseData {
        ResponseData {
            bytes,
            status_code,
            headers,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn to_vec(self) -> Vec<u8> {
        self.bytes.to_vec()
    }

    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }

    pub fn status_code(&self) -> u16 {
        self.status_code
    }

    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_slice())
    }

    pub fn to_string(&self) -> Result<String, std::str::Utf8Error> {
        std::str::from_utf8(self.as_slice()).map(|s| s.to_string())
    }

    pub fn headers(&self) -> HashMap<String, String> {
        self.headers.clone()
    }
}

use crate::error::S3Error;
use futures::Stream;
use std::fmt;
use std::pin::Pin;

impl fmt::Display for ResponseData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Status code: {}\n Data: {}",
            self.status_code(),
            self.to_string()
                .unwrap_or_else(|_| "Data could not be cast to UTF string".to_string())
        )
    }
}
