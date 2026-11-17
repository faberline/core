//! Upgrade and rollback of the probe objects, in-process (#373).
//!
//! The old render is `UPGRADE_ROLLBACK_OLD_RENDER` when that names a file
//! (written by ci/upgrade-rollback-old-render.sh); otherwise it is the
//! current render, so the test still runs without the file.
//!
//! The apiserver here is a fake: it stores each object by path, merges a
//! status PATCH as a JSON merge patch, and replaces a server-side-apply
//! PATCH. It does NOT prove the server-side-apply merge of a real apiserver.
//! It is not the live proof for core@1.0.0; the live proof is #374 on the
//! shared GKE test cluster.

#![cfg(feature = "controller")]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use kube::client::Body;
use kube::Client;
use serde_json::{json, Value};

use crate::support::upgrade_rollback::{
    current_render, old_render_from_env, reconcile_with, NAMESPACE,
};

/// Stored objects by request path (without `?query` and without `/status`).
type Store = Arc<Mutex<BTreeMap<String, Value>>>;

fn old_render() -> Vec<Value> {
    old_render_from_env().unwrap_or_else(current_render)
}

/// A JSON merge patch: objects merge key by key, other values replace.
fn merge(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(t), Value::Object(p)) => {
            for (key, value) in p {
                merge(t.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
        (t, p) => *t = p.clone(),
    }
}

/// The object that a status PATCH creates when none is stored. The probe gets
/// the metadata and the spec that the typed client needs to read the response
/// back; the status fields are merged in afterwards.
fn created_by_status(object: &str) -> Value {
    if object.ends_with("/rollbackprobes/probe") {
        json!({
            "apiVersion": "service-k8s.e2e/v1",
            "kind": "RollbackProbe",
            "metadata": { "name": "probe", "namespace": NAMESPACE, "uid": "uid-upgrade-rollback" },
            "spec": { "objects": [] },
        })
    } else {
        json!({})
    }
}

/// Answers one request against the store.
fn answer(store: &Store, method: &str, path: &str, body: Value) -> (u16, Value) {
    let mut map = store.lock().expect("store lock");
    match method {
        "PATCH" => {
            if let Some(object) = path.strip_suffix("/status") {
                let entry = map
                    .entry(object.to_string())
                    .or_insert_with(|| created_by_status(object));
                merge(entry, &body);
                (200, entry.clone())
            } else {
                map.insert(path.to_string(), body.clone());
                (200, body)
            }
        }
        "POST" if path.ends_with("/events") => (201, json!({ "metadata": { "name": "e" } })),
        "GET" => match map.get(path) {
            Some(stored) => (200, stored.clone()),
            None => (
                404,
                json!({ "kind": "Status", "status": "Failure", "reason": "NotFound", "code": 404 }),
            ),
        },
        _ => (
            500,
            json!({ "kind": "Status", "status": "Failure", "code": 500 }),
        ),
    }
}

/// A client over a stateful fake apiserver, and the store behind it.
fn stateful_apiserver() -> (Client, Store) {
    let store: Store = Arc::new(Mutex::new(BTreeMap::new()));
    let state = store.clone();
    let service = tower::service_fn(move |req: http::Request<Body>| {
        let state = state.clone();
        async move {
            let method = req.method().to_string();
            let path = req.uri().path().to_string();
            let bytes = req.into_body().collect_bytes().await.unwrap_or_default();
            let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            let (code, response) = answer(&state, &method, &path, body);
            Ok::<_, std::convert::Infallible>(
                http::Response::builder()
                    .status(code)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&response).unwrap_or_default(),
                    ))
                    .unwrap_or_else(|e| panic!("response: {e}")),
            )
        }
    });
    (Client::new(service, NAMESPACE), store)
}

fn snapshot(store: &Store) -> BTreeMap<String, Value> {
    store.lock().expect("store lock").clone()
}

fn probe_key(store: &Store) -> String {
    let map = store.lock().expect("store lock");
    map.keys()
        .find(|k| k.ends_with("/rollbackprobes/probe"))
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "no stored probe; keys: {:?}",
                map.keys().collect::<Vec<_>>()
            )
        })
}

/// The `status` of the stored probe object.
fn status_of_probe(store: &Store) -> Value {
    let key = probe_key(store);
    let map = store.lock().expect("store lock");
    map[&key]["status"].clone()
}

/// Writes one status field by the same status PATCH that the harness uses.
fn put_status_field(store: &Store, field: &str, value: Value) {
    let key = probe_key(store);
    let (code, _) = answer(
        store,
        "PATCH",
        &format!("{key}/status"),
        json!({ "status": { field: value } }),
    );
    assert_eq!(code, 200, "status field write");
}

/// Every object of `before` (except the probe itself) is still stored with the
/// same kind and name, both children exist, and the status counts two objects.
fn assert_kept(before: &BTreeMap<String, Value>, store: &Store) {
    let after = snapshot(store);
    for (key, object) in before {
        if key.ends_with("/rollbackprobes/probe") {
            continue;
        }
        let kept = after
            .get(key)
            .unwrap_or_else(|| panic!("object lost: {key}"));
        assert_eq!(kept["kind"], object["kind"], "kind of {key}");
        assert_eq!(
            kept["metadata"]["name"], object["metadata"]["name"],
            "name of {key}"
        );
    }
    assert!(
        after.keys().any(|k| k.ends_with("/serviceaccounts/probe")),
        "ServiceAccount missing: {:?}",
        after.keys().collect::<Vec<_>>()
    );
    assert!(
        after.keys().any(|k| k.ends_with("/services/probe-client")),
        "Service missing: {:?}",
        after.keys().collect::<Vec<_>>()
    );
    assert_eq!(status_of_probe(store)["renderedObjects"], 2);
}

#[tokio::test]
async fn upgrade_keeps_the_objects_and_the_status_of_the_old_render() {
    let (client, store) = stateful_apiserver();

    reconcile_with(client.clone(), old_render())
        .await
        .expect("the old render applies");
    let before = snapshot(&store);

    reconcile_with(client.clone(), current_render())
        .await
        .expect("the current build reconciles the old render");

    assert_kept(&before, &store);
}

#[tokio::test]
async fn rollback_keeps_the_objects_and_the_status() {
    let (client, store) = stateful_apiserver();

    reconcile_with(client.clone(), old_render())
        .await
        .expect("the old render applies");
    let after_old = snapshot(&store);

    reconcile_with(client.clone(), current_render())
        .await
        .expect("the upgrade reconciles");
    reconcile_with(client.clone(), old_render())
        .await
        .expect("the rollback applies the old render again");

    assert_kept(&after_old, &store);
    assert_eq!(
        snapshot(&store).keys().collect::<Vec<_>>(),
        after_old.keys().collect::<Vec<_>>(),
        "the rollback adds or loses no object"
    );
}

#[tokio::test]
async fn status_written_outside_the_current_build_survives_the_upgrade() {
    let (client, store) = stateful_apiserver();

    reconcile_with(client.clone(), old_render())
        .await
        .expect("the old render applies");
    put_status_field(&store, "legacyField", json!("kept"));

    reconcile_with(client.clone(), current_render())
        .await
        .expect("the upgrade reconciles");

    assert_eq!(status_of_probe(&store)["legacyField"], "kept");
    assert_eq!(status_of_probe(&store)["renderedObjects"], 2);
}

#[tokio::test]
async fn a_failed_write_is_an_error_not_a_silent_pass() {
    let (client, store) = stateful_apiserver();
    let bad = vec![json!({ "apiVersion": "v1", "metadata": { "name": "x" } })];

    let result = reconcile_with(client, bad).await;

    assert!(result.is_err(), "an object without a kind must fail");
    assert!(
        !snapshot(&store)
            .keys()
            .any(|k| k.ends_with("/serviceaccounts/probe")),
        "no ServiceAccount is stored after the failed write"
    );
}
