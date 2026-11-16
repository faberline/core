//! Smoke test of the upgrade-rollback harness (#372): the harness applies every
//! object of a render through the current build's `reconcile_once`, and it
//! loads an old render file that ci/upgrade-rollback-old-render.sh writes.
//! The real upgrade and rollback test is #373.

#![cfg(feature = "controller")]

use serde_json::{json, Value};

use crate::support::upgrade_rollback::{
    current_render, fake_apiserver, load_old_render, reconcile_with, Log, NAMESPACE,
};

/// The apiserver answers every write the harness makes: the child PATCHes, the
/// status PATCH of the CR and the event POST.
fn accepting_apiserver() -> (kube::Client, Log) {
    fake_apiserver(|method, path| match (method, path) {
        ("PATCH", p) if p.ends_with("/rollbackprobes/probe/status") => (
            200,
            json!({
                "apiVersion": "service-k8s.e2e/v1",
                "kind": "RollbackProbe",
                "metadata": { "name": "probe", "namespace": NAMESPACE, "uid": "uid-upgrade-rollback" },
                "spec": { "objects": [] },
            }),
        ),
        ("PATCH", _) => (
            200,
            json!({
                "apiVersion": "v1",
                "kind": "ServiceAccount",
                "metadata": { "name": "x", "namespace": NAMESPACE },
            }),
        ),
        ("POST", p) if p.ends_with("/events") => (201, json!({ "metadata": { "name": "e" } })),
        _ => (
            500,
            json!({ "kind": "Status", "status": "Failure", "code": 500 }),
        ),
    })
}

/// The paths of the child PATCHes, in the order the harness sent them.
fn child_patch_paths(log: &Log) -> Vec<String> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|r| r.method == "PATCH" && !r.path.ends_with("/status"))
        .map(|r| r.path.clone())
        .collect()
}

fn assert_both_children_applied(log: &Log) {
    let paths = child_patch_paths(log);
    let sa = paths
        .iter()
        .filter(|p| p.ends_with("/serviceaccounts/probe"))
        .count();
    let svc = paths
        .iter()
        .filter(|p| p.ends_with("/services/probe-client"))
        .count();
    assert_eq!(sa, 1, "exactly one ServiceAccount PATCH: {paths:?}");
    assert_eq!(svc, 1, "exactly one Service PATCH: {paths:?}");
}

#[tokio::test]
async fn harness_applies_every_object_of_a_render_and_writes_status() {
    let (client, log) = accepting_apiserver();

    reconcile_with(client, current_render())
        .await
        .expect("a render the apiserver accepts reconciles");

    assert_both_children_applied(&log);

    let status_writes: Vec<Value> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.method == "PATCH" && r.path.ends_with("/rollbackprobes/probe/status"))
        .map(|r| r.body.clone())
        .collect();
    assert_eq!(
        status_writes.len(),
        1,
        "exactly one status write: {status_writes:?}"
    );
    assert_eq!(
        status_writes[0]["status"]["renderedObjects"], 2,
        "the status write counts the two rendered objects: {}",
        status_writes[0]
    );
}

#[tokio::test]
async fn old_render_file_is_loaded_and_applied() {
    let path = std::env::temp_dir().join("upgrade-rollback-harness-old.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&current_render()).unwrap(),
    )
    .expect("write the old render file");

    let objects = load_old_render(&path);
    assert_eq!(objects.len(), 2, "the file holds the two probe objects");
    assert_eq!(objects[0]["kind"], "ServiceAccount");

    let (client, log) = accepting_apiserver();
    reconcile_with(client, objects)
        .await
        .expect("a render loaded from a file reconciles like one built in code");

    assert_both_children_applied(&log);
}

#[test]
#[should_panic(expected = "not a JSON array of objects")]
fn a_malformed_old_render_is_rejected() {
    let path = std::env::temp_dir().join("upgrade-rollback-harness-bad.json");
    std::fs::write(&path, r#"{"kind":"ServiceAccount"}"#).expect("write the bad file");

    load_old_render(&path);
}
