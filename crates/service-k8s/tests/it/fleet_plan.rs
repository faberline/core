//! Fleet reconciliation planning integration test.

#![cfg(feature = "controller")]

use serde_json::json;
use service_k8s::fleet::{
    desired_hash, plan_fleet, AdoptionPolicy, ChildKey, DesiredChild, FleetAction, FleetPlanError,
    ObservedChild, Ownership, PrunePolicy,
};

#[test]
fn duplicate_desired_targets_fail_fast() {
    let key = ChildKey::namespaced("apps/v1", "Deployment", "tenant-a", "web");
    let desired = vec![
        DesiredChild::new(key.clone(), "hash1"),
        DesiredChild::new(key.clone(), "hash2"),
    ];
    let observed = Vec::<ObservedChild>::new();

    let err = plan_fleet(
        desired,
        observed,
        AdoptionPolicy::Adopt,
        PrunePolicy::Delete,
    )
    .expect_err("duplicate desired targets must fail");

    assert_eq!(err, FleetPlanError::DuplicateTarget(key));
}

#[test]
fn foreign_owned_object_is_skipped_and_survives_delete_policy() {
    let foreign_key = ChildKey::namespaced("apps/v1", "Deployment", "tenant-b", "worker");
    let desired = Vec::<DesiredChild>::new();
    let observed = vec![ObservedChild::foreign(foreign_key.clone(), "other-fleet")];

    let plan = plan_fleet(
        desired,
        observed,
        AdoptionPolicy::Adopt,
        PrunePolicy::Delete,
    )
    .expect("plan succeeds");

    assert_eq!(
        plan.actions,
        vec![FleetAction::SkipForeign { key: foreign_key }]
    );
}

#[test]
fn adoption_of_unowned_object_under_adopt_and_refusal_under_refuse() {
    let key = ChildKey::namespaced("v1", "ConfigMap", "tenant-a", "app-config");
    let hash = "abc123hash";

    // 1. Under AdoptionPolicy::Adopt -> action is Adopt with desired_hash
    let desired = vec![DesiredChild::new(key.clone(), hash)];
    let observed = vec![ObservedChild::unowned(key.clone())];
    let plan_adopt = plan_fleet(
        desired,
        observed,
        AdoptionPolicy::Adopt,
        PrunePolicy::Retain,
    )
    .expect("plan succeeds");
    assert_eq!(
        plan_adopt.actions,
        vec![FleetAction::Adopt {
            key: key.clone(),
            desired_hash: hash.to_string(),
        }]
    );

    // 2. Under AdoptionPolicy::Refuse -> action is SkipForeign
    let desired = vec![DesiredChild::new(key.clone(), hash)];
    let observed = vec![ObservedChild::unowned(key.clone())];
    let plan_refuse = plan_fleet(
        desired,
        observed,
        AdoptionPolicy::Refuse,
        PrunePolicy::Retain,
    )
    .expect("plan succeeds");
    assert_eq!(plan_refuse.actions, vec![FleetAction::SkipForeign { key }]);
}

#[test]
fn retained_orphan_under_retain_is_reported_and_not_pruned() {
    let key = ChildKey::namespaced("v1", "Secret", "tenant-c", "credentials");
    let desired = Vec::<DesiredChild>::new();
    let observed = vec![ObservedChild::owned(key.clone(), "old-hash")];

    let plan = plan_fleet(
        desired,
        observed,
        AdoptionPolicy::Adopt,
        PrunePolicy::Retain,
    )
    .expect("plan succeeds");

    assert_eq!(plan.actions, vec![FleetAction::Orphan { key }]);
}

#[test]
fn same_orphan_under_delete_is_pruned() {
    let key = ChildKey::namespaced("v1", "Secret", "tenant-c", "credentials");
    let desired = Vec::<DesiredChild>::new();
    let observed = vec![ObservedChild::owned(key.clone(), "old-hash")];

    let plan = plan_fleet(
        desired,
        observed,
        AdoptionPolicy::Adopt,
        PrunePolicy::Delete,
    )
    .expect("plan succeeds");

    assert_eq!(plan.actions, vec![FleetAction::Prune { key }]);
}

#[test]
fn unchanged_and_update_by_hash() {
    let key1 = ChildKey::namespaced("apps/v1", "Deployment", "tenant-a", "dep-unchanged");
    let key2 = ChildKey::namespaced("apps/v1", "Deployment", "tenant-a", "dep-update");

    let desired = vec![
        DesiredChild::new(key1.clone(), "hash-match"),
        DesiredChild::new(key2.clone(), "hash-new"),
    ];
    let observed = vec![
        ObservedChild::owned(key1.clone(), "hash-match"),
        ObservedChild::owned(key2.clone(), "hash-old"),
    ];

    let plan = plan_fleet(
        desired,
        observed,
        AdoptionPolicy::Adopt,
        PrunePolicy::Delete,
    )
    .expect("plan succeeds");

    assert_eq!(
        plan.actions,
        vec![
            FleetAction::Unchanged { key: key1 },
            FleetAction::Update {
                key: key2,
                desired_hash: "hash-new".to_string(),
            },
        ]
    );
}

#[test]
fn plan_order_is_stable_under_shuffled_input() {
    let k1 = ChildKey::namespaced("v1", "ConfigMap", "ns-a", "a");
    let k2 = ChildKey::namespaced("v1", "ConfigMap", "ns-a", "b");
    let k3 = ChildKey::namespaced("v1", "ConfigMap", "ns-b", "a");
    let k4 = ChildKey::namespaced("v1", "Secret", "ns-a", "a");

    let expected_order = vec![k1.clone(), k2.clone(), k3.clone(), k4.clone()];

    // Pass inputs in forward order
    let desired_fwd = vec![
        DesiredChild::new(k1.clone(), "h1"),
        DesiredChild::new(k2.clone(), "h2"),
        DesiredChild::new(k3.clone(), "h3"),
        DesiredChild::new(k4.clone(), "h4"),
    ];
    let plan_fwd = plan_fleet(
        desired_fwd,
        Vec::<ObservedChild>::new(),
        AdoptionPolicy::Adopt,
        PrunePolicy::Delete,
    )
    .expect("plan succeeds");

    // Pass inputs in reversed / shuffled order
    let desired_shuffled = vec![
        DesiredChild::new(k3.clone(), "h3"),
        DesiredChild::new(k1.clone(), "h1"),
        DesiredChild::new(k4.clone(), "h4"),
        DesiredChild::new(k2.clone(), "h2"),
    ];
    let plan_shuffled = plan_fleet(
        desired_shuffled,
        Vec::<ObservedChild>::new(),
        AdoptionPolicy::Adopt,
        PrunePolicy::Delete,
    )
    .expect("plan succeeds");

    let order_fwd: Vec<ChildKey> = plan_fwd.actions.iter().map(|a| a.key().clone()).collect();
    let order_shuffled: Vec<ChildKey> = plan_shuffled
        .actions
        .iter()
        .map(|a| a.key().clone())
        .collect();

    assert_eq!(order_fwd, expected_order);
    assert_eq!(order_shuffled, expected_order);
}

#[test]
fn hash_is_key_order_independent() {
    let obj1 = json!({
        "metadata": {
            "name": "sample",
            "labels": {
                "b": "second",
                "a": "first"
            }
        },
        "spec": {
            "replicas": 3,
            "template": {
                "z": 100,
                "y": 200
            }
        }
    });

    let obj2 = json!({
        "spec": {
            "template": {
                "y": 200,
                "z": 100
            },
            "replicas": 3
        },
        "metadata": {
            "labels": {
                "a": "first",
                "b": "second"
            },
            "name": "sample"
        }
    });

    let obj_diff = json!({
        "metadata": {
            "name": "sample",
            "labels": {
                "a": "first",
                "b": "different"
            }
        },
        "spec": {
            "replicas": 3,
            "template": {
                "z": 100,
                "y": 200
            }
        }
    });

    let h1 = desired_hash(&obj1);
    let h2 = desired_hash(&obj2);
    let h3 = desired_hash(&obj_diff);

    assert_eq!(h1, h2, "hashes must be equal regardless of key order");
    assert_ne!(h1, h3, "hashes must differ when values differ");
    assert_eq!(h1.len(), 64, "SHA-256 hex string must be 64 characters");
}

#[test]
fn ownership_derivation_from_label_and_ssa_manager() {
    assert_eq!(
        Ownership::derive(Some("my-fleet"), Some("my-mgr"), "my-fleet", "my-mgr"),
        Ownership::Owned
    );
    assert_eq!(
        Ownership::derive(Some("other-fleet"), Some("my-mgr"), "my-fleet", "my-mgr"),
        Ownership::Foreign {
            owner: "other-fleet".to_string()
        }
    );
    assert_eq!(
        Ownership::derive(None, Some("my-mgr"), "my-fleet", "my-mgr"),
        Ownership::Owned
    );
    assert_eq!(
        Ownership::derive(None, Some("other-mgr"), "my-fleet", "my-mgr"),
        Ownership::Foreign {
            owner: "other-mgr".to_string()
        }
    );
    assert_eq!(
        Ownership::derive(None, None, "my-fleet", "my-mgr"),
        Ownership::Unowned
    );
}
