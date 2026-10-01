//! What a [`BackupSink`](super::BackupSink) reports when a write or a prune
//! fails.

use std::error::Error;

/// A failed [`put`](super::BackupSink::put) or
/// [`prune`](super::BackupSink::prune).
///
/// The adapter's error is kept whole: `Display` and `source()` are the
/// adapter's own, so `{}` and the `{:#}` chain read exactly as the adapter
/// wrote them.
#[derive(Debug, thiserror::Error)]
pub enum BackupSinkError {
    /// The adapter's error, with its cause chain.
    #[error(transparent)]
    Other(Box<dyn Error + Send + Sync>),
}

impl BackupSinkError {
    /// Wrap an adapter's error: an `anyhow::Error`, an `io::Error`, or a
    /// message.
    pub fn other(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self::Other(error.into())
    }
}
