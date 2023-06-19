use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

pub type AuthResult<T> = Result<T, AuthError>;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("The credentials supplied were insufficient to perform the operation requested")]
    Unauthorized,
    #[error("The credentials supplied were invalid")]
    InvalidCredentials,
    #[error("Encountered uncategorized error")]
    ServiceError(#[from] anyhow::Error),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            AuthError::Unauthorized => StatusCode::UNAUTHORIZED.into_response(),
            AuthError::InvalidCredentials => StatusCode::UNAUTHORIZED.into_response(),
            AuthError::ServiceError(error) => {
                tracing::error!(?error, "Encountered service error in auth operation");

                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
