use thiserror::Error;

#[derive(Debug, Error)]
pub enum ObjectStoreError {
    #[error("object {key} was not found")]
    NotFound { key: String },
    #[error("object {key} did not satisfy the write precondition")]
    PreconditionFailed { key: String },
    #[error("object key is invalid: {key}")]
    InvalidKey { key: String },
    #[error("object path is unsafe: {path}")]
    UnsafePath { path: String },
    #[error("object-store authorization failed")]
    Unauthorized,
    #[error("object store is temporarily unavailable: {message}")]
    Unavailable { message: String },
    #[error("object-store response is corrupt: {message}")]
    Corrupt { message: String },
    #[error("object-store I/O failed: {message}")]
    Io { message: String },
}

pub type Result<T> = std::result::Result<T, ObjectStoreError>;
