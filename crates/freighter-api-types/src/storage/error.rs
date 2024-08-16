use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::io;
use thiserror::Error;

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Crate was not found in the storage medium")]
    NotFound,
    #[error("Encountered uncategorized error")]
    ServiceError(#[from] anyhow::Error),
}

impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::NotFound => Self::NotFound,
            _ => Self::ServiceError(e.into()),
        }
    }
}

impl IntoResponse for StorageError {
    fn into_response(self) -> Response {
        let code = match &self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::ServiceError(error) => {
                tracing::error!(?error, "Encountered service error in storage operation");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (code, self.to_string()).into_response()
    }
}
