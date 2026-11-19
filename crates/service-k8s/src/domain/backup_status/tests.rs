use super::*;

fn manifest(id: &str, location: &str) -> BackupManifestRef {
    BackupManifestRef {
        id: id.to_string(),
        location: location.to_string(),
    }
}

fn verified_status() -> BackupStatus {
    next_backup_status(
        &BackupStatus::default(),
        &BackupJobOutcome::Succeeded {
            manifest: Some(manifest("m-1", "gs://bucket/m-1")),
        },
        1000,
    )
}

#[test]
fn default_status_has_no_latest_and_no_attempt() {
    let s = BackupStatus::default();
    assert_eq!(s.latest_successful, None);
    assert_eq!(last_backup_succeeded(&s), None);
    assert_eq!(backup_staleness_seconds(&s, 1000), None);
}

#[test]
fn verified_manifest_sets_latest() {
    let s = verified_status();
    assert_eq!(
        s.latest_successful,
        Some(LatestBackup {
            manifest: manifest("m-1", "gs://bucket/m-1"),
            verified_at_unix: 1000,
        })
    );
    assert_eq!(s.last_attempt, Some(BackupAttempt::Verified));
    assert_eq!(last_backup_succeeded(&s), Some(true));
}

#[test]
fn failed_job_leaves_latest_unchanged() {
    let before = verified_status();
    let s = next_backup_status(
        &before,
        &BackupJobOutcome::Failed {
            reason: "deadline exceeded".into(),
        },
        2000,
    );
    assert_eq!(s.latest_successful, before.latest_successful);
    assert_eq!(
        s.last_attempt,
        Some(BackupAttempt::Failed {
            reason: "deadline exceeded".into(),
        })
    );
    assert_eq!(last_backup_succeeded(&s), Some(false));
}

#[test]
fn succeeded_job_without_manifest_leaves_latest_unchanged() {
    let before = verified_status();
    let s = next_backup_status(
        &before,
        &BackupJobOutcome::Succeeded { manifest: None },
        2000,
    );
    assert_eq!(s.latest_successful, before.latest_successful);
    assert_eq!(s.last_attempt, Some(BackupAttempt::MissingManifest));
    assert_eq!(last_backup_succeeded(&s), Some(false));
}

#[test]
fn unverified_manifest_is_not_accepted() {
    let long_id = "a".repeat(129);
    let cases = [
        ("", "gs://b/m"),
        ("a b", "gs://b/m"),
        ("m-1", "no-scheme"),
        ("m-1", "://x"),
        ("m-1", "gs://"),
        (long_id.as_str(), "gs://b/m"),
    ];
    for (id, location) in cases {
        let m = manifest(id, location);
        assert!(!m.is_verified(), "id={id:?} location={location:?}");
        let s = next_backup_status(
            &BackupStatus::default(),
            &BackupJobOutcome::Succeeded { manifest: Some(m) },
            1000,
        );
        assert_eq!(s.latest_successful, None, "id={id:?} location={location:?}");
        assert_eq!(
            s.last_attempt,
            Some(BackupAttempt::MissingManifest),
            "id={id:?} location={location:?}"
        );
    }
    assert!(manifest("m-1", "gs://bucket/m-1").is_verified());
}

#[test]
fn active_job_changes_nothing() {
    let before = verified_status();
    assert_eq!(
        next_backup_status(&before, &BackupJobOutcome::Active, 2000),
        before
    );
}

#[test]
fn staleness_counts_seconds_since_last_verified_backup() {
    let s = verified_status();
    assert_eq!(backup_staleness_seconds(&s, 1000), Some(0));
    assert_eq!(backup_staleness_seconds(&s, 1600), Some(600));
    assert_eq!(backup_staleness_seconds(&s, 900), Some(0));
}

#[test]
fn new_verified_manifest_replaces_the_older_latest() {
    let first = verified_status();
    let s = next_backup_status(
        &first,
        &BackupJobOutcome::Succeeded {
            manifest: Some(manifest("m-2", "gs://bucket/m-2")),
        },
        2000,
    );
    assert_eq!(
        s.latest_successful,
        Some(LatestBackup {
            manifest: manifest("m-2", "gs://bucket/m-2"),
            verified_at_unix: 2000,
        })
    );
}
