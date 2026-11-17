//! The `status.conditions` facts of one profile instance: five standard conditions that
//! every instance has, then the dimensions the app reports itself.
//!
//! Pure and clock-free. `project` in `condition.rs` stamps the times later.

use crate::domain::condition::{ConditionFact, ConditionStatus};

/// The five standard condition types, in the order `instance_conditions` returns them.
pub const STANDARD_CONDITION_TYPES: [&str; 5] = [
    "Ready",
    "Progressing",
    "StorageHealthy",
    "BackupConfigured",
    "BackupSucceeded",
];

/// The app's own readiness dimensions, for example `ShardMapHealthy`.
pub trait DomainReadiness {
    /// The extra condition facts of this instance, in the order they must appear.
    fn dimensions(&self) -> Vec<ConditionFact>;
}

/// What the controller observed about one instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstanceFacts {
    /// Replicas that are ready now.
    pub ready_replicas: u32,
    /// Replicas the spec asks for.
    pub desired_replicas: u32,
    /// `true` when every volume is healthy.
    pub storage_healthy: bool,
    /// `true` when the instance has a backup schedule.
    pub backup_configured: bool,
    /// `None` before the first backup ran, else whether the last one succeeded.
    pub last_backup_succeeded: Option<bool>,
}

fn status(flag: bool) -> ConditionStatus {
    if flag {
        ConditionStatus::True
    } else {
        ConditionStatus::False
    }
}

/// The five standard facts (in the order of [`STANDARD_CONDITION_TYPES`]), then the facts of
/// `domain` in the order the app gave them. A domain fact whose `type_` is a standard type
/// is dropped: the standard one wins.
pub fn instance_conditions(
    facts: &InstanceFacts,
    domain: &dyn DomainReadiness,
) -> Vec<ConditionFact> {
    let ready = facts.ready_replicas == facts.desired_replicas && facts.desired_replicas > 0;
    let rolling = facts.ready_replicas != facts.desired_replicas;
    let replicas = format!(
        "{}/{} replicas ready",
        facts.ready_replicas, facts.desired_replicas
    );

    let backup_succeeded = match facts.last_backup_succeeded {
        None => ConditionFact::new(
            "BackupSucceeded",
            ConditionStatus::Unknown,
            "NoBackupYet",
            "no backup has run yet",
        ),
        Some(true) => ConditionFact::new(
            "BackupSucceeded",
            ConditionStatus::True,
            "LastBackupSucceeded",
            "the last backup succeeded",
        ),
        Some(false) => ConditionFact::new(
            "BackupSucceeded",
            ConditionStatus::False,
            "LastBackupFailed",
            "the last backup failed",
        ),
    };

    let mut out = vec![
        ConditionFact::new(
            "Ready",
            status(ready),
            if ready {
                "AllReplicasReady"
            } else {
                "ReplicasNotReady"
            },
            replicas.clone(),
        ),
        ConditionFact::new(
            "Progressing",
            status(rolling),
            if rolling { "Rolling" } else { "Idle" },
            replicas,
        ),
        ConditionFact::new(
            "StorageHealthy",
            status(facts.storage_healthy),
            if facts.storage_healthy {
                "StorageOk"
            } else {
                "StorageDegraded"
            },
            if facts.storage_healthy {
                "storage is healthy"
            } else {
                "storage is degraded"
            },
        ),
        ConditionFact::new(
            "BackupConfigured",
            status(facts.backup_configured),
            if facts.backup_configured {
                "BackupConfigured"
            } else {
                "NoBackup"
            },
            if facts.backup_configured {
                "a backup is configured"
            } else {
                "no backup is configured"
            },
        ),
        backup_succeeded,
    ];
    out.extend(
        domain
            .dimensions()
            .into_iter()
            .filter(|fact| !STANDARD_CONDITION_TYPES.contains(&fact.type_.as_str())),
    );
    out
}

#[cfg(test)]
mod tests;
