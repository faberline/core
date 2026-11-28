//! Tests of the kube adapter of the `FleetCluster` port (#678). A route-based
//! fake apiserver records every request; each port call runs inside
//! `spawn_blocking`, because the port calls `Handle::block_on`.

use std::sync::{Arc, Mutex};

use kube::client::Body;
use serde_json::{json, Value};

use super::*;
use crate::domain::fleet::Ownership;

const SET_BASE: &str = "/apis/apps/v1/namespaces/tenant-a/statefulsets";

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    query: String,
    content_type: String,
    body: Value,
}

type Seen = Arc<Mutex<Vec<Recorded>>>;

fn fake(route: impl Fn(&str, &str) -> (u16, Value) + Send + Sync + 'static) -> (Client, Seen) {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let route = Arc::new(route);
    let service = tower::service_fn(move |req: http::Request<Body>| {
        let log = log.clone();
        let route = route.clone();
        async move {
            let method = req.method().to_string();
            let path = req.uri().path().to_string();
            let query = req.uri().query().unwrap_or("").to_string();
            let content_type = req
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_string();
            let bytes = req.into_body().collect_bytes().await.unwrap_or_default();
            let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            let (code, response) = route(&method, &path);
            log.lock().unwrap().push(Recorded {
                method,
                path,
                query,
                content_type,
                body,
            });
            Ok::<_, std::convert::Infallible>(
                http::Response::builder()
                    .status(code)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&response).unwrap()))
                    .unwrap(),
            )
        }
    });
    (Client::new(service, "default"), seen)
}

/// Run one closure on a cluster adapter, the way the Fleet pass does.
async fn run<T: Send + 'static>(
    client: Client,
    namespaces: Option<Vec<String>>,
    call: impl FnOnce(&mut KubeFleetCluster) -> T + Send + 'static,
) -> T {
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        let mut cluster = KubeFleetCluster::new(client, handle, namespaces);
        call(&mut cluster)
    })
    .await
    .unwrap()
}

fn key(name: &str) -> ChildKey {
    ChildKey::namespaced("apps/v1", "StatefulSet", "tenant-a", name)
}

fn not_found() -> Value {
    json!({"kind": "Status", "status": "Failure", "reason": "NotFound", "code": 404})
}

fn internal() -> Value {
    json!({"kind": "Status", "status": "Failure", "reason": "InternalError", "code": 500})
}

fn sts(name: &str, namespace: &str, owner: Option<&str>, hash: Option<&str>) -> Value {
    let mut metadata = json!({"name": name, "namespace": namespace});
    if let Some(owner) = owner {
        metadata["labels"] = json!({"faberline.dev/fleet-owner": owner});
    }
    if let Some(hash) = hash {
        metadata["annotations"] = json!({"faberline.dev/desired-hash": hash});
    }
    json!({"apiVersion": "apps/v1", "kind": "StatefulSet", "metadata": metadata})
}

fn list_of(items: Vec<Value>) -> Value {
    json!({
        "apiVersion": "apps/v1",
        "kind": "StatefulSetList",
        "metadata": {"resourceVersion": "1"},
        "items": items,
    })
}

// 1. Owned, unowned, foreign, missing, and an owned object found only by the list.
fn route_observed(method: &str, path: &str) -> (u16, Value) {
    match (method, path.strip_prefix(SET_BASE)) {
        ("GET", Some("/a")) => (200, sts("a", "tenant-a", Some("fleet-a"), Some("h1"))),
        ("GET", Some("/b")) => (200, sts("b", "tenant-a", None, None)),
        ("GET", Some("/c")) => (200, sts("c", "tenant-a", Some("other"), None)),
        ("GET", Some("/gone")) => (404, not_found()),
        ("GET", Some("")) => (
            200,
            list_of(vec![
                sts("stale", "tenant-a", Some("fleet-a"), Some("h0")),
                sts("a", "tenant-a", Some("fleet-a"), Some("h1")),
            ]),
        ),
        _ => (500, internal()),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn observed_classifies_owned_unowned_and_foreign() {
    let (client, seen) = fake(route_observed);
    let keys = vec![key("a"), key("b"), key("c"), key("gone")];

    let observed = run(client, Some(Vec::new()), move |cluster| {
        cluster.observed("fleet-a", &keys)
    })
    .await
    .unwrap();

    assert_eq!(
        observed,
        vec![
            ObservedChild::new(key("a"), Ownership::Owned, Some("h1")),
            ObservedChild::new(key("b"), Ownership::Unowned, None::<String>),
            ObservedChild::new(
                key("c"),
                Ownership::Foreign {
                    owner: "other".to_string()
                },
                None::<String>
            ),
            ObservedChild::new(
                ChildKey::namespaced("apps/v1", "StatefulSet", "tenant-a", "stale"),
                Ownership::Owned,
                Some("h0"),
            ),
        ]
    );
    let list = seen
        .lock()
        .unwrap()
        .iter()
        .find(|r| r.method == "GET" && r.path == SET_BASE)
        .cloned()
        .expect("one list call in tenant-a");
    assert!(
        list.query.contains("labelSelector="),
        "query: {}",
        list.query
    );
    assert!(list.query.contains("fleet-owner"), "query: {}", list.query);
    assert!(list.query.contains("fleet-a"), "query: {}", list.query);
}

// 2. A namespace list makes no cluster-wide request.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn observed_with_a_namespace_list_makes_no_cluster_wide_request() {
    let (client, seen) = fake(|_, path| {
        if path.ends_with("/statefulsets") {
            (200, list_of(Vec::new()))
        } else {
            (404, not_found())
        }
    });
    let keys = vec![key("web")];

    let result = run(client, Some(vec!["team-a".to_string()]), move |cluster| {
        cluster.observed("fleet-a", &keys)
    })
    .await;

    assert_eq!(result.unwrap(), Vec::new());
    let seen = seen.lock().unwrap();
    assert!(!seen.is_empty());
    for request in seen.iter() {
        assert!(
            request.path.starts_with("/apis/apps/v1/namespaces/team-a/")
                || request
                    .path
                    .starts_with("/apis/apps/v1/namespaces/tenant-a/"),
            "unexpected path: {}",
            request.path
        );
        assert_ne!(request.path, "/apis/apps/v1/statefulsets");
    }
}

// 3. No namespace list means one cluster-wide list per kind.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn observed_with_none_lists_cluster_wide() {
    let (client, seen) = fake(|_, path| {
        if path.ends_with("/statefulsets") {
            (200, list_of(Vec::new()))
        } else {
            (404, not_found())
        }
    });
    let keys = vec![key("web")];

    let result = run(client, None, move |cluster| {
        cluster.observed("fleet-a", &keys)
    })
    .await;

    assert_eq!(result.unwrap(), Vec::new());
    let seen = seen.lock().unwrap();
    let cluster_wide = seen
        .iter()
        .find(|r| r.method == "GET" && r.path == "/apis/apps/v1/statefulsets")
        .expect("one cluster-wide list");
    assert!(cluster_wide.query.contains("labelSelector="));
}

// 4. A key without a namespace is not listed, even with a namespace list.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn observed_skips_listing_for_keys_without_a_namespace() {
    let (client, seen) = fake(|_, _| (404, not_found()));
    let keys = vec![ChildKey::new("apps/v1", "StatefulSet", None::<String>, "x")];

    let result = run(client, Some(Vec::new()), move |cluster| {
        cluster.observed("fleet-a", &keys)
    })
    .await;

    assert_eq!(result.unwrap(), Vec::new());
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "only the get of the key: {seen:?}");
    assert_eq!(seen[0].path, "/apis/apps/v1/statefulsets/x");
}

// 5. A failing get is reported with the verb and the key.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn observed_reports_a_failing_read_with_the_verb_and_the_key() {
    let (client, _seen) = fake(|_, _| (500, internal()));
    let keys = vec![key("a")];

    let error = run(client, Some(Vec::new()), move |cluster| {
        cluster.observed("fleet-a", &keys)
    })
    .await
    .unwrap_err();

    assert!(
        error
            .0
            .starts_with("observe apps/v1/StatefulSet/tenant-a/a: "),
        "error: {}",
        error.0
    );
}

// 6. Apply sends a server-side apply with the owner label and the hash.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_sends_server_side_apply_with_owner_label_and_hash() {
    let (client, seen) = fake(|_, _| (200, sts("web", "tenant-a", Some("fleet-a"), Some("h1"))));

    let result = run(client, None, move |cluster| {
        cluster.apply(
            &key("web"),
            &json!({"replicas": 1}),
            "h1",
            "fleet-a",
            "faberline-fleet",
        )
    })
    .await;

    assert!(result.is_ok(), "apply: {result:?}");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let request = &seen[0];
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.path, format!("{SET_BASE}/web"));
    assert!(
        request.query.contains("fieldManager=faberline-fleet"),
        "query: {}",
        request.query
    );
    assert!(
        request.query.contains("force=true"),
        "query: {}",
        request.query
    );
    assert_eq!(request.content_type, "application/apply-patch+yaml");
    assert_eq!(request.body["apiVersion"], "apps/v1");
    assert_eq!(request.body["kind"], "StatefulSet");
    assert_eq!(
        request.body["metadata"]["labels"]["faberline.dev/fleet-owner"],
        "fleet-a"
    );
    assert_eq!(
        request.body["metadata"]["annotations"]["faberline.dev/desired-hash"],
        "h1"
    );
    assert_eq!(request.body["spec"], json!({"replicas": 1}));
}

// 7. Apply without a namespace leaves the namespace out of the body.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_without_a_namespace_leaves_it_out_of_the_body() {
    let (client, seen) = fake(|_, _| (200, sts("web", "tenant-a", Some("fleet-a"), Some("h1"))));
    let cluster_key = ChildKey::new("apps/v1", "StatefulSet", None::<String>, "web");

    let result = run(client, None, move |cluster| {
        cluster.apply(
            &cluster_key,
            &json!({"replicas": 1}),
            "h1",
            "fleet-a",
            "faberline-fleet",
        )
    })
    .await;

    assert!(result.is_ok(), "apply: {result:?}");
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].path, "/apis/apps/v1/statefulsets/web");
    assert!(seen[0].body["metadata"].get("namespace").is_none());
}

// 8. A failing apply is reported with the verb and the key.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_and_adopt_report_failures_with_the_verb_and_the_key() {
    let (client, _seen) = fake(|_, _| (500, internal()));

    let apply = run(client, None, move |cluster| {
        cluster.apply(&key("web"), &json!({}), "h1", "fleet-a", "faberline-fleet")
    })
    .await
    .unwrap_err();
    assert!(
        apply
            .0
            .starts_with("apply apps/v1/StatefulSet/tenant-a/web: "),
        "{}",
        apply.0
    );

    let (client, _seen) = fake(|_, _| (500, internal()));
    let adopt = run(client, None, move |cluster| {
        cluster.adopt(&key("web"), "h1", "fleet-a")
    })
    .await
    .unwrap_err();
    assert!(
        adopt
            .0
            .starts_with("adopt apps/v1/StatefulSet/tenant-a/web: "),
        "{}",
        adopt.0
    );
}

// 9. Adopt merges only the owner label and the hash.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adopt_merges_only_the_label_and_the_annotation() {
    let (client, seen) = fake(|_, _| (200, sts("web", "tenant-a", Some("fleet-a"), Some("h1"))));

    let result = run(client, None, move |cluster| {
        cluster.adopt(&key("web"), "h1", "fleet-a")
    })
    .await;

    assert!(result.is_ok(), "adopt: {result:?}");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, "PATCH");
    assert_eq!(seen[0].content_type, "application/merge-patch+json");
    assert_eq!(
        seen[0].body,
        json!({
            "metadata": {
                "labels": {"faberline.dev/fleet-owner": "fleet-a"},
                "annotations": {"faberline.dev/desired-hash": "h1"}
            }
        })
    );
}

// 10. Delete treats a 404 as done and names the key on any other failure.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_treats_not_found_as_done_and_names_the_key_on_failure() {
    let (client, _seen) = fake(|_, _| (404, not_found()));
    let gone = run(client, None, move |cluster| cluster.delete(&key("web"))).await;
    assert!(gone.is_ok(), "delete of a missing child: {gone:?}");

    let (client, _seen) = fake(|_, _| (500, internal()));
    let failed = run(client, None, move |cluster| cluster.delete(&key("web")))
        .await
        .unwrap_err();
    assert!(
        failed.0.contains("delete apps/v1/StatefulSet/tenant-a/web"),
        "error: {}",
        failed.0
    );
}

// 11. Readiness needs enough ready replicas and an observed generation.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn is_ready_needs_ready_replicas_and_an_observed_generation() {
    // (spec.replicas, status.readyReplicas, status.observedGeneration, metadata.generation)
    type Case = (Option<i64>, Option<i64>, Option<i64>, Option<i64>, bool);
    let cases: [Case; 4] = [
        (Some(3), Some(3), Some(2), Some(2), true),
        (Some(3), Some(2), Some(2), Some(2), false),
        (Some(3), Some(3), Some(1), Some(2), false),
        (Some(1), None, None, Some(1), false),
    ];
    for (replicas, ready, observed, generation, expected) in cases {
        let mut object = sts("web", "tenant-a", Some("fleet-a"), Some("h1"));
        if let Some(replicas) = replicas {
            object["spec"] = json!({"replicas": replicas});
        }
        let mut status = json!({});
        if let Some(ready) = ready {
            status["readyReplicas"] = json!(ready);
        }
        if let Some(observed) = observed {
            status["observedGeneration"] = json!(observed);
        }
        object["status"] = status;
        if let Some(generation) = generation {
            object["metadata"]["generation"] = json!(generation);
        }
        let (client, _seen) = fake(move |_, _| (200, object.clone()));
        let got = run(client, None, move |cluster| cluster.is_ready(&key("web"))).await;
        assert_eq!(
            got.unwrap(),
            expected,
            "case {replicas:?} {ready:?} {observed:?} {generation:?}"
        );
    }

    let (client, _seen) = fake(|_, _| (404, not_found()));
    let absent = run(client, None, move |cluster| cluster.is_ready(&key("web"))).await;
    assert!(!absent.unwrap());

    let (client, _seen) = fake(|_, _| (500, internal()));
    let failed = run(client, None, move |cluster| cluster.is_ready(&key("web")))
        .await
        .unwrap_err();
    assert!(
        failed
            .0
            .starts_with("probe apps/v1/StatefulSet/tenant-a/web: "),
        "{}",
        failed.0
    );
}

// 12. The resource plural is the one the apiserver serves.
#[test]
fn api_for_uses_the_plural_the_apiserver_serves() {
    assert_eq!(
        api_resource("apps/v1", "StatefulSet").plural,
        "statefulsets"
    );
    assert_eq!(
        api_resource("networking.k8s.io/v1", "NetworkPolicy").plural,
        "networkpolicies"
    );
    assert_eq!(api_resource("apps/v1", "StatefulSet").group, "apps");
    assert_eq!(api_resource("v1", "ConfigMap").group, "");
}
