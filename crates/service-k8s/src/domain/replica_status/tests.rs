use super::*;
use serde_json::json;

#[test]
fn phase_is_ready_when_all_desired_replicas_are_ready() {
    assert_eq!(replica_phase(3, 3), ReplicaPhase::Ready);
    assert_eq!(replica_phase(3, 4), ReplicaPhase::Ready);
}

#[test]
fn phase_is_reconciling_when_some_replicas_are_ready() {
    assert_eq!(replica_phase(3, 1), ReplicaPhase::Reconciling);
}

#[test]
fn phase_is_pending_when_no_replica_is_ready() {
    assert_eq!(replica_phase(3, 0), ReplicaPhase::Pending);
}

#[test]
fn zero_desired_replicas_is_never_ready() {
    assert_eq!(replica_phase(0, 0), ReplicaPhase::Pending);
    assert_eq!(replica_phase(0, 2), ReplicaPhase::Reconciling);
}

#[test]
fn phase_strings_match_the_crd_values() {
    assert_eq!(ReplicaPhase::Pending.as_str(), "Pending");
    assert_eq!(ReplicaPhase::Reconciling.as_str(), "Reconciling");
    assert_eq!(ReplicaPhase::Ready.as_str(), "Ready");
}

#[test]
fn message_has_the_ready_over_desired_form() {
    assert_eq!(replicas_message(1, 2, "store pods"), "1/2 store pods ready");
}

#[test]
fn flat_patch_has_exactly_the_five_status_fields() {
    let patch = flat_status_patch(7, 2, 3, "2/3 broker pods ready");
    assert_eq!(
        patch,
        json!({"status": {
            "phase": "Reconciling",
            "observedGeneration": 7,
            "readyReplicas": 2,
            "desiredReplicas": 3,
            "message": "2/3 broker pods ready"
        }})
    );
    assert_eq!(patch["status"].as_object().unwrap().len(), 5);
}
