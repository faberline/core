use super::*;
use serde_json::{json, Value};

fn sts(name: &str, selector: Value, claim: &str) -> Value {
    json!({
        "metadata": { "name": name },
        "spec": {
            "selector": { "matchLabels": selector },
            "volumeClaimTemplates": [{ "metadata": { "name": claim } }]
        }
    })
}

fn old_selector() -> Value {
    json!({ "app.kubernetes.io/instance": "s" })
}

fn new_selector() -> Value {
    json!({ "app.kubernetes.io/instance": "s", "shard": "0" })
}

fn shape(value: &Value) -> StatefulSetShape {
    StatefulSetShape::from_value(value).expect("fixture is a valid statefulset")
}

#[test]
fn unlabeled_statefulset_is_recreated_with_orphan() {
    let observed = shape(&sts("s", old_selector(), "raft"));
    let desired = shape(&sts("s", new_selector(), "raft"));

    assert_eq!(
        plan_shard_migration(&observed, &desired, 1),
        ShardMigrationPlan::RecreateOrphan
    );
}

#[test]
fn statefulset_with_shard_selector_needs_no_change() {
    let live = shape(&sts("s", new_selector(), "raft"));
    let desired = shape(&sts("s", new_selector(), "raft"));

    // A shard label means the new layout, so the multi-shard refusal does not apply.
    assert_eq!(
        plan_shard_migration(&live, &desired, 3),
        ShardMigrationPlan::NoChange
    );
}

#[test]
fn changed_claim_template_name_is_refused() {
    let observed = shape(&sts("s", old_selector(), "raft"));
    let desired = shape(&sts("s", new_selector(), "data"));

    match plan_shard_migration(&observed, &desired, 1) {
        ShardMigrationPlan::Refuse(msg) => {
            assert!(
                msg.contains("volumeClaimTemplates"),
                "refusal must name the claim templates, got: {msg}"
            );
        }
        other => panic!("expected Refuse, got {other:?}"),
    }
}

#[test]
fn different_name_is_refused() {
    let observed = shape(&sts("s", old_selector(), "raft"));
    let desired = shape(&sts("t", new_selector(), "raft"));

    match plan_shard_migration(&observed, &desired, 1) {
        ShardMigrationPlan::Refuse(msg) => {
            assert!(
                msg.contains("name differs"),
                "refusal must say the name differs, got: {msg}"
            );
        }
        other => panic!("expected Refuse, got {other:?}"),
    }
}

#[test]
fn multi_shard_old_layout_is_refused() {
    let observed = shape(&sts("s", old_selector(), "raft"));
    let desired = shape(&sts("s", new_selector(), "raft"));

    assert_eq!(
        plan_shard_migration(&observed, &desired, 2),
        ShardMigrationPlan::Refuse("multi-shard ordinal layout: not migrated".to_string())
    );
}

#[test]
fn pvc_names_follow_claim_then_ordinal() {
    let one = shape(&sts("s", old_selector(), "raft"));
    assert_eq!(one.pvc_names(2), vec!["raft-s-0", "raft-s-1"]);

    let two = StatefulSetShape::from_value(&json!({
        "metadata": { "name": "s" },
        "spec": {
            "selector": { "matchLabels": old_selector() },
            "volumeClaimTemplates": [
                { "metadata": { "name": "a" } },
                { "metadata": { "name": "b" } }
            ]
        }
    }))
    .expect("two claim templates");
    assert_eq!(two.pvc_names(1), vec!["a-s-0", "b-s-0"]);
}

#[test]
fn from_value_requires_name_and_selector() {
    let no_name = json!({
        "spec": { "selector": { "matchLabels": old_selector() } }
    });
    assert_eq!(
        StatefulSetShape::from_value(&no_name),
        Err(ShardMigrationError::Missing("metadata.name"))
    );

    let no_selector = json!({ "metadata": { "name": "s" }, "spec": {} });
    assert_eq!(
        StatefulSetShape::from_value(&no_selector),
        Err(ShardMigrationError::Missing("spec.selector.matchLabels"))
    );

    let no_claims = json!({
        "metadata": { "name": "s" },
        "spec": { "selector": { "matchLabels": old_selector() } }
    });
    let parsed = StatefulSetShape::from_value(&no_claims).expect("claims are optional");
    assert!(parsed.claim_templates.is_empty());
}
