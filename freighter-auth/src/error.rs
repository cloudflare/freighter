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
