use std::time::SystemTime;

use anyhow::Result;

use crate::infrastructure::fetch_admin_snapshot;
use crate::{
    run_backup_once, sink_from_destination, BackupDestination, BackupRunResult, RetentionPolicy,
};

/// Fetch an admin snapshot and ship the exact bytes to `dest`.
///
/// `file://` always works. `s3://` requires the crate's `s3` feature; `gs://`
/// remains schema-compatible and fails loudly until a GCS sink exists.
pub async fn run_admin_snapshot_backup(
    base_url: &str,
    token: Option<&str>,
    dest: &BackupDestination,
    retention: &RetentionPolicy,
) -> Result<BackupRunResult> {
    let payload = fetch_admin_snapshot(base_url, token).await?;
    let sink = sink_from_destination(dest)?;
    run_backup_once(sink.as_ref(), SystemTime::now(), &payload, retention)
}
