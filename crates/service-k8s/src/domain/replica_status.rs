//! Replica status of a managed service: the phase, the message and the flat
//! status patch that the operators write. Pure: no API call and no clock.

use serde_json::json;

#[cfg(test)]
mod tests;

/// The phase of a replica set, as the CRD status reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplicaPhase {
    /// No replica is ready.
    Pending,
    /// Some replicas are ready, but not all of the desired ones.
    Reconciling,
    /// All desired replicas are ready. Zero desired replicas is never ready.
    Ready,
}

impl ReplicaPhase {
    /// The CRD value of the phase.
    pub fn as_str(self) -> &'static str {
        match self {
            ReplicaPhase::Pending => "Pending",
            ReplicaPhase::Reconciling => "Reconciling",
            ReplicaPhase::Ready => "Ready",
        }
    }
}

/// The phase for the desired and the ready replica counts.
pub fn replica_phase(desired_replicas: i32, ready_replicas: i32) -> ReplicaPhase {
    if desired_replicas > 0 && ready_replicas >= desired_replicas {
        ReplicaPhase::Ready
    } else if ready_replicas > 0 {
        ReplicaPhase::Reconciling
    } else {
        ReplicaPhase::Pending
    }
}

/// The message `"{ready}/{desired} {noun} ready"`.
pub fn replicas_message(ready_replicas: i32, desired_replicas: i32, noun: &str) -> String {
    format!("{ready_replicas}/{desired_replicas} {noun} ready")
}

/// The status patch with the five flat status fields.
pub fn flat_status_patch(
    observed_generation: i64,
    ready_replicas: i32,
    desired_replicas: i32,
    message: &str,
) -> serde_json::Value {
    json!({ "status": {
        "phase": replica_phase(desired_replicas, ready_replicas).as_str(),
        "observedGeneration": observed_generation,
        "readyReplicas": ready_replicas,
        "desiredReplicas": desired_replicas,
        "message": message,
    }})
}
