//! The port a backup run writes snapshot bytes through.

use std::time::SystemTime;

use super::BackupSinkError;

/// Destination for snapshot bytes.
///
/// The trait is synchronous. The adapters (the local filesystem, GCS and S3
/// sinks) live in the crate's infrastructure layer and keep their own error
/// context; they hand it over whole as a [`BackupSinkError`].
pub trait BackupSink: Send + Sync + 'static {
    /// Store bytes under a key derived from `timestamp`; returns the final key.
    fn put(&self, timestamp: SystemTime, payload: &[u8]) -> Result<String, BackupSinkError>;

    /// Apply age retention and return number of objects removed.
    fn prune(&self, max_age_seconds: u64) -> Result<usize, BackupSinkError>;

    /// Human-readable sink identity for logs/status.
    fn identity(&self) -> String;
}
