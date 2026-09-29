use thiserror::Error;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("invalid text-index schema: {message}")]
    InvalidSchema { message: String },
    #[error("unknown text-index field: {field}")]
    UnknownField { field: String },
    #[error("field {field} does not support {operation}")]
    UnsupportedFieldOperation {
        field: String,
        operation: &'static str,
    },
    #[error("invalid text-index document: {message}")]
    InvalidDocument { message: String },
    #[error("text-index snapshot is corrupt: {message}")]
    CorruptSnapshot { message: String },
    #[error("text-index lock is poisoned")]
    LockPoisoned,
}

pub type Result<T> = std::result::Result<T, IndexError>;
