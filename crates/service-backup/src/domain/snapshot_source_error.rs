//! What a [`SnapshotSource`](super::SnapshotSource) reports when a fetch
//! fails.

use std::error::Error;

/// A failed [`fetch_snapshot`](super::SnapshotSource::fetch_snapshot).
///
/// The adapter's error is kept whole: `Display` and `source()` are the
/// adapter's own.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotSourceError {
    /// The adapter's error, with its cause chain.
    #[error(transparent)]
    Other(Box<dyn Error + Send + Sync>),
}

impl SnapshotSourceError {
    /// Wrap an adapter's error.
    pub fn other(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self::Other(error.into())
    }
}
