use thiserror::Error;

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Crate was not found in the storage medium")]
    NotFound,
    #[error("Failed to upload because crate was already present")]
    UploadConflict,
    #[error("Encountered uncategorized error")]
    ServerError(#[from] anyhow::Error),
}
