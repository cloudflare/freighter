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

impl IntoResponse for IndexError {
    fn into_response(self) -> Response {
        match self {
            IndexError::Conflict(s) => {
                tracing::error!("Encountered conflict in index operation: {s}");

                StatusCode::CONFLICT.into_response()
            }
            IndexError::NotFound => StatusCode::NOT_FOUND.into_response(),
            IndexError::CrateNameNotAllowed => StatusCode::BAD_REQUEST.into_response(),
            IndexError::ServiceError(error) => {
                tracing::error!(?error, "Encountered service error in index operation");

                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
