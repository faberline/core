//! Pure planner for one scheduled backup Job per schedule slot.
//!
//! It decides whether a backup Job may start, given the Jobs already observed
//! for the service. It makes no API call: the caller lists the Jobs of one
//! service, passes them here, and creates the Job when told to start.

#[cfg(test)]
mod tests;

/// Retry, deadline and cleanup values for a backup Job. The caller supplies
/// them; the library has no implicit value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackupJobPolicy {
    pub backoff_limit: i32,
    pub active_deadline_seconds: i64,
    pub ttl_seconds_after_finished: i32,
}

/// The state of an observed backup Job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupJobPhase {
    Active,
    Succeeded,
    Failed,
}

/// One backup Job the caller found for the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedBackupJob {
    pub name: String,
    pub phase: BackupJobPhase,
}

/// What the caller should do for one schedule slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackupJobDecision {
    Start { job_name: String },
    WaitForActive { job_name: String },
    AlreadyRan { job_name: String },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BackupJobError {
    #[error(
        "backup slot {0:?} must be 1-40 chars of a-z, 0-9 and '-', not starting or ending with '-'"
    )]
    InvalidSlot(String),
    #[error("backup job name {name:?} is {len} chars; the limit is 63")]
    NameTooLong { name: String, len: usize },
}

/// The Job name for one service and one schedule slot.
pub fn backup_job_name(service: &str, slot: &str) -> Result<String, BackupJobError> {
    let valid_slot = !slot.is_empty()
        && slot.len() <= 40
        && slot
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !slot.starts_with('-')
        && !slot.ends_with('-');
    if !valid_slot {
        return Err(BackupJobError::InvalidSlot(slot.to_string()));
    }
    let name = format!("{service}-backup-{slot}");
    if name.len() > 63 {
        return Err(BackupJobError::NameTooLong {
            len: name.len(),
            name,
        });
    }
    Ok(name)
}

/// Decides what to do for `slot`, given the Jobs already observed for `service`.
///
/// Two reconciles that see the same Jobs choose the same name, so the second
/// `create` gets HTTP 409, and the caller treats 409 as `AlreadyRan`.
pub fn plan_backup_job(
    service: &str,
    slot: &str,
    observed: &[ObservedBackupJob],
) -> Result<BackupJobDecision, BackupJobError> {
    let name = backup_job_name(service, slot)?;
    let active = observed
        .iter()
        .filter(|job| job.phase == BackupJobPhase::Active)
        .map(|job| job.name.as_str())
        .min();
    if let Some(job_name) = active {
        return Ok(BackupJobDecision::WaitForActive {
            job_name: job_name.to_string(),
        });
    }
    if observed.iter().any(|job| job.name == name) {
        return Ok(BackupJobDecision::AlreadyRan { job_name: name });
    }
    Ok(BackupJobDecision::Start { job_name: name })
}
