//! Fleet entry model: targets, duplicate rejection, rollout order and aggregation.

#![cfg(feature = "controller")]

use serde_json::json;
use service_k8s::fleet::{
    aggregate_fleet_condition, plan_rollout_order, reject_duplicate_targets, ChildKey,
    FleetAggregate, FleetEntry, FleetEntryPhase, FleetEntryStatus, FleetPlanError, FleetTarget,
    RolloutState,
};

fn entry(target: FleetTarget, name: Option<&str>) -> FleetEntry {
    FleetEntry {
        target,
        name: name.map(str::to_string),
        spec: json!({}),
    }
}

fn status(target: FleetTarget, phase: FleetEntryPhase) -> FleetEntryStatus {
    FleetEntryStatus {
        target,
        phase,
        observed_hash: None,
        message: None,
    }
}

#[test]
fn duplicate_target_in_same_cluster_is_rejected() {
    let entries = vec![
        entry(FleetTarget::local("tape-app"), Some("a")),
        entry(FleetTarget::local("tape-app"), Some("a")),
    ];

    let err = reject_duplicate_targets(&entries, "v1", "ConfigMap", "x").unwrap_err();

    assert_eq!(
        err,
        FleetPlanError::DuplicateTarget(ChildKey::namespaced("v1", "ConfigMap", "tape-app", "a"))
    );
}

#[test]
fn none_and_local_cluster_collide() {
    let entries = vec![
        entry(FleetTarget::new(None, "ns"), Some("a")),
        entry(FleetTarget::new(Some("local".to_string()), "ns"), Some("a")),
    ];

    let err = reject_duplicate_targets(&entries, "v1", "ConfigMap", "x").unwrap_err();

    assert_eq!(
        err,
        FleetPlanError::DuplicateTarget(ChildKey::namespaced("v1", "ConfigMap", "ns", "a"))
    );
}

#[test]
fn same_key_in_two_clusters_is_not_a_duplicate() {
    let entries = vec![
        entry(FleetTarget::new(Some("uat-1".to_string()), "ns"), Some("a")),
        entry(
            FleetTarget::new(Some("prod-1".to_string()), "ns"),
            Some("a"),
        ),
    ];

    assert_eq!(
        reject_duplicate_targets(&entries, "v1", "ConfigMap", "x"),
        Ok(())
    );
}

#[test]
fn default_name_is_used_when_name_is_none() {
    let e = entry(FleetTarget::local("ns"), None);

    let key = e.child_key("v1", "ConfigMap", "x");

    assert_eq!(key.name, "x");
    assert_eq!(key.namespace.as_deref(), Some("ns"));
}

#[test]
fn cluster_none_and_omitted_serialize_the_same() {
    let none = serde_json::to_value(FleetTarget::local("ns")).unwrap();
    assert_eq!(none, json!({"namespace": "ns"}));

    let back: FleetTarget = serde_json::from_value(json!({"namespace": "ns"})).unwrap();
    assert_eq!(back, FleetTarget::local("ns"));

    let remote = serde_json::to_value(FleetTarget::new(Some("uat-1".to_string()), "ns")).unwrap();
    assert_eq!(remote, json!({"cluster": "uat-1", "namespace": "ns"}));
}

#[test]
fn rollout_order_respects_max_concurrent() {
    let states = [
        RolloutState::Pending,
        RolloutState::Pending,
        RolloutState::Pending,
    ];

    assert_eq!(plan_rollout_order(&states, 2), vec![0, 1]);
}

#[test]
fn rollout_order_counts_updating_entries() {
    let states = [
        RolloutState::Updating,
        RolloutState::Pending,
        RolloutState::Pending,
    ];

    assert_eq!(plan_rollout_order(&states, 2), vec![1]);
}

#[test]
fn rollout_order_one_at_a_time_holds_behind_updating() {
    let states = [
        RolloutState::Ready,
        RolloutState::Updating,
        RolloutState::Pending,
    ];

    assert!(plan_rollout_order(&states, 1).is_empty());
}

#[test]
fn rollout_order_zero_slots_returns_empty() {
    let states = [RolloutState::Pending];

    assert!(plan_rollout_order(&states, 0).is_empty());
}

#[test]
fn aggregate_all_ready_and_empty() {
    assert_eq!(
        aggregate_fleet_condition(&[]),
        FleetAggregate {
            ready: true,
            message: "no entries".to_string(),
        }
    );

    let ready = [
        status(FleetTarget::local("a"), FleetEntryPhase::Ready),
        status(FleetTarget::local("b"), FleetEntryPhase::Ready),
    ];
    assert_eq!(
        aggregate_fleet_condition(&ready),
        FleetAggregate {
            ready: true,
            message: "all entries ready".to_string(),
        }
    );
}

#[test]
fn aggregate_names_first_failed_or_unreachable() {
    let entries = [
        status(FleetTarget::local("a"), FleetEntryPhase::Pending),
        status(
            FleetTarget::new(Some("uat-1".to_string()), "tape-app"),
            FleetEntryPhase::Unreachable,
        ),
        status(FleetTarget::local("b"), FleetEntryPhase::Failed),
    ];

    assert_eq!(
        aggregate_fleet_condition(&entries),
        FleetAggregate {
            ready: false,
            message: "Unreachable entry uat-1/tape-app".to_string(),
        }
    );
}

#[test]
fn aggregate_names_first_not_ready_when_none_failed() {
    let entries = [
        status(FleetTarget::local("a"), FleetEntryPhase::Ready),
        status(FleetTarget::local("b"), FleetEntryPhase::Applying),
    ];

    assert_eq!(
        aggregate_fleet_condition(&entries),
        FleetAggregate {
            ready: false,
            message: "Applying entry local/b".to_string(),
        }
    );
}

#[test]
fn entry_types_do_not_name_an_app() {
    let sources = [
        include_str!("../../src/domain/fleet/target.rs"),
        include_str!("../../src/domain/fleet/entry.rs"),
        include_str!("../../src/domain/fleet/rollout_order.rs"),
        include_str!("../../src/domain/fleet/status.rs"),
    ];
    let banned = [
        concat!("Lum", "en"),
        concat!("Ta", "pe"),
        concat!("Si", "ft"),
    ];

    for source in sources {
        for word in banned {
            assert!(!source.contains(word), "new fleet source names {word}");
        }
    }
}
