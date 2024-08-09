#[cfg(feature = "storage")]
use crate::storage::StorageError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IndexError {
    #[error("A resource conflict occurred while attempting an operation: {0}")]
    Conflict(String),
    #[error("Requested a crate with a name that is too long (64) or contains non-ASCII characters or punctuation")]
    CrateNameNotAllowed,
    #[error("Failed to find the resource")]
    NotFound,
    #[error("Encountered uncategorized error")]
    ServiceError(#[from] anyhow::Error),
}

#[cfg(feature = "storage")]
impl From<StorageError> for IndexError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::NotFound => Self::NotFound,
            StorageError::ServiceError(e) => Self::ServiceError(e),
        }
    }
}

#[cfg(feature = "index")]
impl From<serde_json::Error> for IndexError {
    fn from(error: serde_json::Error) -> Self {
        IndexError::ServiceError(error.into())
    }
}

impl IntoResponse for IndexError {
    fn into_response(self) -> Response {
        let code = match &self {
            IndexError::Conflict(s) => {
                tracing::error!("Encountered conflict in index operation: {s}");

                StatusCode::CONFLICT
            }
            IndexError::NotFound => StatusCode::NOT_FOUND,
            IndexError::CrateNameNotAllowed => StatusCode::BAD_REQUEST,
            IndexError::ServiceError(error) => {
                tracing::error!(?error, "Encountered service error in index operation");

                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (code, self.to_string()).into_response()
    }
}
