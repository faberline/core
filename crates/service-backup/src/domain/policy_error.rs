use super::DestinationError;

/// Why [`ScheduledBackupPolicy::to_runtime_policy`](crate::ScheduledBackupPolicy::to_runtime_policy)
/// rejected a policy.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    /// `schedule` is empty or only whitespace.
    #[error("backup schedule must not be empty")]
    EmptySchedule,
    /// `destination` is not a URI `from_uri` accepts. The message is the
    /// destination error's own, with nothing added.
    #[error(transparent)]
    Destination(#[from] DestinationError),
}
