//! Pure planner for one restore Job per restore request.
//!
//! It decides whether a restore Job may start, and whether readiness stays
//! gated until the service acknowledges that the domain restore is complete.
//! It makes no API call and holds no data: the manifest is a reference only.

use crate::domain::backup_status::BackupManifestRef;

#[cfg(test)]
mod tests;

/// Finalizer that keeps a restore Job until its handoff is finished.
pub const RESTORE_FINALIZER: &str = "service-k8s.axiom.dev/restore-handoff";

/// Retry, deadline and cleanup values for a restore Job. The caller supplies
/// them; the library has no implicit value.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreJobPolicy {
    pub backoff_limit: i32,
    pub active_deadline_seconds: i64,
    pub ttl_seconds_after_finished: i32,
}

/// What the caller asks for: a restore id and the explicit manifest to restore.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreRequest {
    /// 1-40 characters of a-z, 0-9 and '-', not starting or ending with '-'.
    pub restore_id: String,
    pub manifest: BackupManifestRef,
}

/// The state of an observed restore Job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreJobPhase {
    Active,
    Succeeded,
    Failed,
}

/// One restore Job the caller found for the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedRestoreJob {
    pub name: String,
    pub phase: RestoreJobPhase,
}

/// What the caller should do for one restore request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreJobDecision {
    Start { job_name: String },
    WaitForActive { job_name: String },
    AlreadyRan { job_name: String },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RestoreJobError {
    #[error(
        "restore id {0:?} must be 1-40 chars of a-z, 0-9 and '-', not starting or ending with '-'"
    )]
    InvalidRestoreId(String),
    #[error("restore job name {name:?} is {len} chars; the limit is 63")]
    NameTooLong { name: String, len: usize },
    #[error("restore manifest is not verified: id and location must be set")]
    UnverifiedManifest,
}

/// The observed outcome of one restore Job, as mapped by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreJobOutcome {
    NotStarted,
    Active,
    Failed { reason: String },
    Succeeded,
}

/// Where one restore is in its handoff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestorePhase {
    /// No Job yet.
    Pending,
    /// The Job is active.
    Running,
    /// The Job failed (backoffLimit or deadline spent).
    Failed { reason: String },
    /// The Job succeeded; the service has not acknowledged the domain restore.
    AwaitingAck,
    /// The Job succeeded and the service acknowledged it. Terminal.
    Complete,
}

/// The status of one restore request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreStatus {
    pub restore_id: String,
    pub manifest: BackupManifestRef,
    pub phase: RestorePhase,
}

/// The Job name for one service and one restore id.
pub fn restore_job_name(service: &str, restore_id: &str) -> Result<String, RestoreJobError> {
    let valid_id = !restore_id.is_empty()
        && restore_id.len() <= 40
        && restore_id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !restore_id.starts_with('-')
        && !restore_id.ends_with('-');
    if !valid_id {
        return Err(RestoreJobError::InvalidRestoreId(restore_id.to_string()));
    }
    let name = format!("{service}-restore-{restore_id}");
    if name.len() > 63 {
        return Err(RestoreJobError::NameTooLong {
            len: name.len(),
            name,
        });
    }
    Ok(name)
}

/// Decides what to do for `request`, given the Jobs already observed for
/// `service` and the status of the restore, if any.
pub fn plan_restore_job(
    service: &str,
    request: &RestoreRequest,
    observed: &[ObservedRestoreJob],
    status: Option<&RestoreStatus>,
) -> Result<RestoreJobDecision, RestoreJobError> {
    let name = restore_job_name(service, &request.restore_id)?;
    if !request.manifest.is_verified() {
        return Err(RestoreJobError::UnverifiedManifest);
    }
    if let Some(status) = status {
        if status.restore_id == request.restore_id && status.phase == RestorePhase::Complete {
            return Ok(RestoreJobDecision::AlreadyRan { job_name: name });
        }
    }
    let active = observed
        .iter()
        .filter(|job| job.phase == RestoreJobPhase::Active)
        .map(|job| job.name.as_str())
        .min();
    if let Some(job_name) = active {
        return Ok(RestoreJobDecision::WaitForActive {
            job_name: job_name.to_string(),
        });
    }
    if observed.iter().any(|job| job.name == name) {
        return Ok(RestoreJobDecision::AlreadyRan { job_name: name });
    }
    Ok(RestoreJobDecision::Start { job_name: name })
}

/// The next status after one observed Job outcome.
pub fn next_restore_status(
    previous: Option<&RestoreStatus>,
    request: &RestoreRequest,
    outcome: &RestoreJobOutcome,
    acknowledged: bool,
) -> RestoreStatus {
    if let Some(previous) = previous {
        if previous.restore_id == request.restore_id && previous.phase == RestorePhase::Complete {
            return previous.clone();
        }
    }
    let phase = match outcome {
        RestoreJobOutcome::NotStarted => RestorePhase::Pending,
        RestoreJobOutcome::Active => RestorePhase::Running,
        RestoreJobOutcome::Failed { reason } => RestorePhase::Failed {
            reason: reason.clone(),
        },
        RestoreJobOutcome::Succeeded if acknowledged => RestorePhase::Complete,
        RestoreJobOutcome::Succeeded => RestorePhase::AwaitingAck,
    };
    RestoreStatus {
        restore_id: request.restore_id.clone(),
        manifest: request.manifest.clone(),
        phase,
    }
}

/// True while readiness must stay gated by the restore.
pub fn restore_gates_readiness(status: Option<&RestoreStatus>) -> bool {
    match status {
        None => false,
        Some(status) => status.phase != RestorePhase::Complete,
    }
}

/// True when the finalizer may be removed.
pub fn restore_finalizer_removable(status: &RestoreStatus) -> bool {
    matches!(
        status.phase,
        RestorePhase::Complete | RestorePhase::Failed { .. }
    )
}
