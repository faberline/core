use super::reconcile_metrics::CountedService;
use super::*;
use crate::domain::retention::INSTANCE_UID_LABEL;
use crate::interfaces::operator::children::{
    apply_object, other_namespace_finalizer, place_child, prune_other_namespace_object,
    ClusterPruneOutcome,
};
use crate::service::OtherNamespaceChild;
use std::collections::BTreeMap;

/// A ConfigMap named `shared`, in `namespace` when it is `Some`.
fn cm(namespace: Option<&str>) -> Value {
    let mut metadata = json!({ "name": "shared" });
    if let Some(ns) = namespace {
        metadata["namespace"] = json!(ns);
    }
    json!({ "apiVersion": "v1", "kind": "ConfigMap", "metadata": metadata })
}

/// A live ConfigMap in `other-a` with the owner UID label `uid_label`, the
/// expected label `app` when `expected` is `Some`, and one SSA manager.
fn live_cm(uid_label: &str, expected: Option<&str>, manager: &str) -> Value {
    let mut labels = json!({ INSTANCE_UID_LABEL: uid_label });
    if let Some(value) = expected {
        labels["app"] = json!(value);
    }
    json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {
            "name": "shared",
            "namespace": "other-a",
            "uid": "cm-uid",
            "resourceVersion": "7",
            "labels": labels,
            "managedFields": [{
                "manager": manager,
                "operation": "Apply",
                "apiVersion": "v1",
            }],
        },
    })
}

#[test]
fn other_namespace_finalizer_names_the_manager() {
    assert_eq!(
        other_namespace_finalizer::<CountedService>(),
        "service-k8s.axiom.dev/counted-operator-other-namespace-children"
    );
}

/// The prune target: a ConfigMap in `other-a` that must carry `app=x`.
fn other_target() -> OtherNamespaceChild {
    OtherNamespaceChild::new("v1", "ConfigMap", "other-a", "shared", true)
        .with_expected_labels(BTreeMap::from([("app".to_string(), "x".to_string())]))
}

#[tokio::test]
async fn apply_object_uses_the_namespace_of_the_child() {
    let (client, seen) = fake_apiserver(vec![(200, cm(Some("other-a")))]);
    apply_object(&client, "acme", "mgr", cm(Some("other-a")))
        .await
        .expect("apply succeeds");
    assert_eq!(
        *seen.lock().unwrap(),
        ["PATCH /api/v1/namespaces/other-a/configmaps/shared"]
    );
}

#[tokio::test]
async fn apply_object_without_a_namespace_uses_the_owner_namespace() {
    let (client, seen) = fake_apiserver(vec![(200, cm(Some("acme")))]);
    apply_object(&client, "acme", "mgr", cm(None))
        .await
        .expect("apply succeeds");
    assert_eq!(
        *seen.lock().unwrap(),
        ["PATCH /api/v1/namespaces/acme/configmaps/shared"]
    );
}

#[test]
fn place_child_keeps_an_owner_namespace_child() {
    let declared: [OtherNamespaceChild; 0] = [];
    assert_eq!(
        place_child(cm(None), "acme", "u-1", &declared).expect("placed"),
        cm(None)
    );
    assert_eq!(
        place_child(cm(Some("acme")), "acme", "u-1", &declared).expect("placed"),
        cm(Some("acme"))
    );
}

#[test]
fn place_child_refuses_an_undeclared_namespace() {
    let err = place_child(cm(Some("other-a")), "acme", "u-1", &[]).unwrap_err();
    match err {
        Error::Plan(msg) => {
            assert!(msg.contains("other-a"), "{msg}");
            assert!(msg.contains("other_namespace_children"), "{msg}");
        }
        other => panic!("expected Error::Plan, got {other:?}"),
    }
}

#[test]
fn place_child_refuses_a_child_declared_as_not_desired() {
    let not_desired = OtherNamespaceChild::new("v1", "ConfigMap", "other-a", "shared", false);
    let other_ns = OtherNamespaceChild::new("v1", "ConfigMap", "other-b", "shared", true);
    for declared in [not_desired, other_ns] {
        let err = place_child(cm(Some("other-a")), "acme", "u-1", &[declared]).unwrap_err();
        assert!(matches!(err, Error::Plan(_)), "{err:?}");
    }
}

#[test]
fn place_child_stamps_labels_and_drops_owner_references() {
    let mut child = cm(Some("other-a"));
    child["metadata"]["ownerReferences"] = json!([{ "uid": "u-owner" }]);
    child["metadata"]["labels"] = json!({ "app": "x" });
    let declared = [other_target()];
    let placed = place_child(child, "acme", "u-1", &declared).expect("placed");
    assert!(
        placed["metadata"].get("ownerReferences").is_none(),
        "{placed}"
    );
    assert_eq!(
        placed["metadata"]["labels"],
        json!({ INSTANCE_UID_LABEL: "u-1", "app": "x" })
    );
    assert_eq!(placed["metadata"]["namespace"], "other-a");
}

#[tokio::test]
async fn prune_other_namespace_deletes_an_owned_child() {
    let (client, seen) = fake_apiserver(vec![
        (200, live_cm("u-1", Some("x"), "mgr")),
        (200, live_cm("u-1", Some("x"), "mgr")),
    ]);
    let outcome = prune_other_namespace_object(&client, "mgr", "u-1", &other_target())
        .await
        .expect("prune succeeds");
    assert_eq!(outcome, ClusterPruneOutcome::DeleteRequested);
    assert_eq!(
        *seen.lock().unwrap(),
        [
            "GET /api/v1/namespaces/other-a/configmaps/shared",
            "DELETE /api/v1/namespaces/other-a/configmaps/shared",
        ]
    );
}

#[tokio::test]
async fn prune_other_namespace_of_an_absent_child_is_absent() {
    let (client, seen) = fake_apiserver(vec![not_found()]);
    let outcome = prune_other_namespace_object(&client, "mgr", "u-1", &other_target())
        .await
        .expect("absent is success");
    assert_eq!(outcome, ClusterPruneOutcome::Absent);
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn prune_other_namespace_refuses_another_owner_uid() {
    let (client, seen) = fake_apiserver(vec![(200, live_cm("u-other", Some("x"), "mgr"))]);
    let err = prune_other_namespace_object(&client, "mgr", "u-1", &other_target())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::NamespacedOwnership { .. }), "{err:?}");
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn prune_other_namespace_refuses_a_missing_expected_label() {
    let (client, seen) = fake_apiserver(vec![(200, live_cm("u-1", None, "mgr"))]);
    let err = prune_other_namespace_object(&client, "mgr", "u-1", &other_target())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::NamespacedOwnership { .. }), "{err:?}");
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn prune_other_namespace_refuses_a_foreign_field_manager() {
    let (client, seen) = fake_apiserver(vec![(200, live_cm("u-1", Some("x"), "someone-else"))]);
    let err = prune_other_namespace_object(&client, "mgr", "u-1", &other_target())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::NamespacedOwnership { .. }), "{err:?}");
    assert_eq!(seen.lock().unwrap().len(), 1);
}
