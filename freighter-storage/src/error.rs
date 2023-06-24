use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Crate was not found in the storage medium")]
    NotFound,
    #[error("Encountered uncategorized error")]
    ServiceError(#[from] anyhow::Error),
}

impl IntoResponse for StorageError {
    fn into_response(self) -> Response {
        match self {
            StorageError::NotFound => StatusCode::NOT_FOUND.into_response(),
            StorageError::ServiceError(error) => {
                tracing::error!(?error, "Encountered service error in storage operation");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
