//! Pure status of scheduled backups, derived from observed backup Job outcomes.
//!
//! The latest successful backup changes only when a finished Job exposes a
//! verified manifest id and location. It makes no API call and reads no clock:
//! the caller passes the time in.

#[cfg(test)]
mod tests;

/// A backup manifest that a finished Job exposed: its id and where it lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupManifestRef {
    /// Manifest id, 1 to 128 characters with no whitespace.
    pub id: String,
    /// Location URL with a scheme, such as `gs://bucket/path`.
    pub location: String,
}

impl BackupManifestRef {
    /// True only when the id is 1 to 128 characters with no whitespace and the
    /// location has a non-empty part before and after `://`.
    pub fn is_verified(&self) -> bool {
        let id_ok = (1..=128).contains(&self.id.chars().count())
            && !self.id.chars().any(char::is_whitespace);
        let location_ok = match self.location.split_once("://") {
            Some((scheme, rest)) => !scheme.is_empty() && !rest.is_empty(),
            None => false,
        };
        id_ok && location_ok
    }
}

/// The observed outcome of one backup Job, as mapped by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackupJobOutcome {
    /// The Job is still running.
    Active,
    /// The Job failed, with the reason it reported.
    Failed { reason: String },
    /// The Job finished; `manifest` is `None` when it exposed none.
    Succeeded { manifest: Option<BackupManifestRef> },
}

/// The last verified backup: its manifest and the time it was verified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatestBackup {
    /// The verified manifest.
    pub manifest: BackupManifestRef,
    /// Unix time, in seconds, when the backup was verified.
    pub verified_at_unix: i64,
}

/// The result of the most recent backup attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackupAttempt {
    /// The attempt produced a verified manifest.
    Verified,
    /// The attempt failed, with the reason.
    Failed { reason: String },
    /// The attempt finished without a verified manifest.
    MissingManifest,
}

/// The backup status of one service.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackupStatus {
    /// The last verified backup, if any.
    pub latest_successful: Option<LatestBackup>,
    /// The most recent attempt, if any.
    pub last_attempt: Option<BackupAttempt>,
}

/// Applies one observed Job outcome to the previous status.
pub fn next_backup_status(
    previous: &BackupStatus,
    outcome: &BackupJobOutcome,
    now_unix: i64,
) -> BackupStatus {
    match outcome {
        BackupJobOutcome::Active => previous.clone(),
        BackupJobOutcome::Failed { reason } => BackupStatus {
            latest_successful: previous.latest_successful.clone(),
            last_attempt: Some(BackupAttempt::Failed {
                reason: reason.clone(),
            }),
        },
        BackupJobOutcome::Succeeded { manifest: Some(m) } if m.is_verified() => BackupStatus {
            latest_successful: Some(LatestBackup {
                manifest: m.clone(),
                verified_at_unix: now_unix,
            }),
            last_attempt: Some(BackupAttempt::Verified),
        },
        BackupJobOutcome::Succeeded { .. } => BackupStatus {
            latest_successful: previous.latest_successful.clone(),
            last_attempt: Some(BackupAttempt::MissingManifest),
        },
    }
}

/// Seconds since the last verified backup, or `None` when there is none.
/// A clock behind the verification time gives 0.
pub fn backup_staleness_seconds(status: &BackupStatus, now_unix: i64) -> Option<i64> {
    status
        .latest_successful
        .as_ref()
        .map(|latest| (now_unix - latest.verified_at_unix).max(0))
}

/// Whether the last attempt succeeded, or `None` when there was no attempt.
pub fn last_backup_succeeded(status: &BackupStatus) -> Option<bool> {
    match status.last_attempt.as_ref()? {
        BackupAttempt::Verified => Some(true),
        BackupAttempt::Failed { .. } | BackupAttempt::MissingManifest => Some(false),
    }
}
