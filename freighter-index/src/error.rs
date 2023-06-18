use thiserror::Error;

#[derive(Error, Debug)]
pub enum IndexError {
    #[error("A resource conflict occurred while attempting an operation: {0}")]
    Conflict(String),
    #[error("Encountered uncategorized error")]
    ServiceError(#[from] anyhow::Error),
}
