use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub type AuthResult<T> = Result<T, AuthError>;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("The credentials were missing, or were insufficient to perform the operation requested")]
    Unauthorized,
    #[error("The client is not allowed to perform the operation requested")]
    Forbidden,
    #[error("The credentials supplied were invalid")]
    InvalidCredentials,
    #[error("This operation is not implemented")]
    Unimplemented,
    #[error("The requested crate does not exist")]
    CrateNotFound,
    #[error("Internal error ({})", error_id(_0))]
    ServiceError(#[from] anyhow::Error),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let code = match &self {
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::CrateNotFound => StatusCode::NOT_FOUND,
            Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::Unimplemented => StatusCode::NOT_IMPLEMENTED,
            Self::ServiceError(error) => {
                tracing::error!(?error, "Encountered service error in auth operation");

                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (code, self.to_string()).into_response()
    }
}

/// We can't disclose the acutal message, it could contian private info or attacker-injected strings.
/// But it is useful to differentiate between different types of internal errors.
fn error_id(err: &anyhow::Error) -> String {
    let msg = err.to_string();
    format!("{:.6x}", Sha256::digest(msg.as_bytes()))
}
