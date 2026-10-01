//! What a [`BearerTokenSource`](super::BearerTokenSource) reports when it
//! cannot produce a token.

use std::error::Error;

/// A token that could not be read or validated.
///
/// The adapter's error is kept whole: `Display` and `source()` are the
/// adapter's own.
#[derive(Debug, thiserror::Error)]
pub enum BearerTokenError {
    /// The adapter's error, with its cause chain.
    #[error(transparent)]
    Other(Box<dyn Error + Send + Sync>),
}

impl BearerTokenError {
    /// Wrap an adapter's error.
    pub fn other(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self::Other(error.into())
    }
}
