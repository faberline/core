//! `run_admin_snapshot_backup`: the strict admin snapshot transport wired to
//! the sink a destination names.

use anyhow::Result;

use super::sink_from_destination;
use crate::application::{run_snapshot_backup, BackupRunResult};
use crate::domain::{BackupDestination, RetentionPolicy};
use crate::infrastructure::{AdminSnapshotEndpoint, AdminSnapshotTransport};

/// Fetch an admin snapshot and ship the exact bytes to `dest`.
///
/// The fetch is one [`AdminSnapshotTransport::fetch_exact`] call with the
/// production limits: 10 s connect, 2 h per operation, 30 s idle between body
/// chunks, no redirects, exactly `200 OK`, and a redacted error that never
/// carries the response body or the token. `file://` and `gs://` always work;
/// `s3://` requires the crate's `s3` feature and otherwise fails loudly.
pub async fn run_admin_snapshot_backup(
    base_url: &str,
    token: Option<&str>,
    dest: &BackupDestination,
    retention: &RetentionPolicy,
) -> Result<BackupRunResult> {
    let transport = AdminSnapshotTransport::new()?;
    let source = AdminSnapshotEndpoint::new(transport, base_url, token);
    run_snapshot_backup(&source, || sink_from_destination(dest), retention).await
}
