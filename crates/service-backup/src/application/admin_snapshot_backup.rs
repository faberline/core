//! The admin-snapshot backup: fetch one snapshot, then write it through a
//! sink.

use std::time::SystemTime;

use anyhow::Result;

use super::{run_backup_once, BackupRunResult};
use crate::domain::{BackupSink, RetentionPolicy, SnapshotSource};

/// Fetch one snapshot from `source` and ship the exact bytes to the sink that
/// `open_sink` opens.
///
/// The sink is opened only after the fetch succeeds, so a failed fetch leaves
/// nothing behind (a local destination's directory is not created).
pub(crate) async fn run_snapshot_backup(
    source: &dyn SnapshotSource,
    open_sink: impl FnOnce() -> Result<Box<dyn BackupSink>>,
    retention: &RetentionPolicy,
) -> Result<BackupRunResult> {
    let payload = source.fetch_snapshot().await?;
    let sink = open_sink()?;
    run_backup_once(sink.as_ref(), SystemTime::now(), &payload, retention)
}
