//! `BackupSinkError` hands an adapter's error over whole: the `{}` text and
//! the `{:#}` chain a caller sees through `run_backup_once` are the adapter's.

use std::time::SystemTime;

use anyhow::anyhow;
use service_backup::{run_backup_once, BackupSink, BackupSinkError, RetentionPolicy};

struct Failing;

impl BackupSink for Failing {
    fn put(&self, _timestamp: SystemTime, _payload: &[u8]) -> Result<String, BackupSinkError> {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "read-only volume");
        Err(BackupSinkError::other(
            anyhow::Error::new(io).context("write /backups/svc-10.json"),
        ))
    }

    fn prune(&self, _max_age_seconds: u64) -> Result<usize, BackupSinkError> {
        Err(BackupSinkError::other(anyhow!("prune refused")))
    }

    fn identity(&self) -> String {
        "failing".into()
    }
}

#[test]
fn a_put_error_keeps_its_text_and_chain() {
    let error = run_backup_once(
        &Failing,
        SystemTime::UNIX_EPOCH,
        b"x",
        &RetentionPolicy::default(),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "write /backups/svc-10.json");
    assert_eq!(
        format!("{error:#}"),
        "write /backups/svc-10.json: read-only volume"
    );
}

#[test]
fn a_sink_error_is_transparent() {
    let error = BackupSinkError::other(anyhow!("inner").context("outer"));
    assert_eq!(error.to_string(), "outer");
    assert_eq!(
        std::error::Error::source(&error).map(ToString::to_string),
        Some("inner".to_string())
    );
    let message = BackupSinkError::other("plain message");
    assert_eq!(message.to_string(), "plain message");
    assert!(std::error::Error::source(&message).is_none());
}

#[cfg(not(feature = "s3"))]
#[test]
fn an_unlinked_s3_sink_error_reads_as_before() {
    let destination = service_backup::BackupDestination::from_uri("s3://bucket/prefix").unwrap();
    let sink = service_backup::sink_from_destination(&destination).unwrap();
    let error = run_backup_once(
        sink.as_ref(),
        SystemTime::UNIX_EPOCH,
        b"x",
        &RetentionPolicy::default(),
    )
    .unwrap_err();
    let expected = "backup destination s3://bucket/prefix requires the service-backup `s3` feature in the runner; rebuild with `--features s3` or use file://";
    assert_eq!(error.to_string(), expected);
    assert_eq!(format!("{error:#}"), expected);
}
