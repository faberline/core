//! The output text of the shared `backup` verb: one backup run from
//! parameters, printed as the pretty JSON of `BackupRunResult`. The services
//! own their flags, env names and exit codes; this module owns the text.

use std::time::SystemTime;

use anyhow::Result;

use super::{run_admin_snapshot_backup, sink_from_destination};
use crate::application::{run_backup_once, BackupRunResult};
use crate::domain::{BackupDestination, RetentionPolicy};

/// How a service marks the end of its `backup` output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextMarker {
    /// stdout is the JSON document only (tape, keep).
    None,
    /// The JSON document, then a separate line `next: done` (relay, defer).
    DoneLine,
    /// The JSON object holds the key `"next": "done"` (sift).
    DoneKey,
}

/// Parameters of a live backup (tape, relay, defer, sift `--url`).
#[derive(Clone, Debug)]
pub struct BackupVerbParams<'a> {
    /// Base URL of the node; the run fetches `{url}/admin/backup`.
    pub url: &'a str,
    /// Optional bearer token.
    pub token: Option<&'a str>,
    /// Destination URI, for example `file:///tmp/backups`.
    pub dest: &'a str,
    /// `Some(secs)` removes backup objects older than `secs`.
    pub retention_secs: Option<u64>,
    /// The marker policy of the calling service.
    pub marker: NextMarker,
}

/// One live backup run; returns the text to print on stdout (no trailing newline).
pub async fn run_backup_verb(params: &BackupVerbParams<'_>) -> Result<String> {
    let dest = BackupDestination::from_uri(params.dest)?;
    let result = run_admin_snapshot_backup(
        params.url,
        params.token,
        &dest,
        &retention(params.retention_secs),
    )
    .await?;
    render_backup_output(&result, params.marker)
}

/// One backup run of bytes the caller already has (the offline source: keep,
/// sift `--data-dir`); returns the text to print on stdout (no trailing newline).
pub async fn run_backup_verb_payload(
    payload: &[u8],
    dest: &str,
    retention_secs: Option<u64>,
    marker: NextMarker,
) -> Result<String> {
    let dest = BackupDestination::from_uri(dest)?;
    let sink = sink_from_destination(&dest)?;
    let result = run_backup_once(
        sink.as_ref(),
        SystemTime::now(),
        payload,
        &retention(retention_secs),
    )?;
    render_backup_output(&result, marker)
}

/// The stdout text of one finished run.
pub fn render_backup_output(result: &BackupRunResult, marker: NextMarker) -> Result<String> {
    match marker {
        NextMarker::None => Ok(serde_json::to_string_pretty(result)?),
        NextMarker::DoneLine => Ok(format!(
            "{}\nnext: done",
            serde_json::to_string_pretty(result)?
        )),
        NextMarker::DoneKey => {
            let mut value = serde_json::to_value(result)?;
            let Some(object) = value.as_object_mut() else {
                anyhow::bail!("backup result is not a JSON object");
            };
            object.insert(
                "next".to_string(),
                serde_json::Value::String("done".to_string()),
            );
            Ok(serde_json::to_string_pretty(&value)?)
        }
    }
}

fn retention(secs: Option<u64>) -> RetentionPolicy {
    RetentionPolicy {
        max_age_seconds: secs,
    }
}
