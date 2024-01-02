#![cfg_attr(docsrs, feature(doc_cfg))]

pub use freighter_api_types::storage::{StorageError, StorageResult};

pub mod s3_client;

pub mod fs;
