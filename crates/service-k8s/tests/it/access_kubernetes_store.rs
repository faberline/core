//! Server-Side Apply of access grant Roles and RoleBindings through the
//! Kubernetes store, against a fake apiserver that records every request.

#![cfg(feature = "controller")]

use std::sync::{Arc, Mutex};

use kube::client::Body;
use kube::Client;
use serde_json::{json, Value};

use service_k8s::trust_bundle::StoreErrorKind;
use service_k8s::{
    reconcile_access, render_access_grants, AccessGrant, AccessObjectKind, AccessRule, AccessStore,
    AccessSubject, KubernetesAccessStore,
};

const BASE: &str = "/apis/rbac.authorization.k8s.io/v1/namespaces";

/// Every request the fake apiserver saw: method, path, query and body text.
type Log = Arc<Mutex<Vec<(String, String, String, String)>>>;

fn fake_apiserver<F>(responder: F) -> (Client, Log)
where
    F: Fn(&str, &str) -> (u16, Value) + Send + Sync + 'static,
{
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let seen = log.clone();
    let responder = Arc::new(responder);
    let service = tower::service_fn(move |req: http::Request<Body>| {
        let seen = seen.clone();
        let responder = responder.clone();
        async move {
            let method = req.method().to_string();
            let path = req.uri().path().to_string();
            let query = req.uri().query().unwrap_or("").to_string();
            let bytes = req.into_body().collect_bytes().await.unwrap_or_default();
            let body = String::from_utf8(bytes.to_vec()).unwrap_or_default();
            let (code, response) = responder(&method, &path);
            seen.lock().unwrap().push((method, path, query, body));
            Ok::<_, std::convert::Infallible>(
                http::Response::builder()
                    .status(code)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&response).unwrap()))
                    .unwrap(),
            )
        }
    });
    (Client::new(service, "app-ns"), log)
}

fn status(code: u16, reason: &str) -> (u16, Value) {
    (
        code,
        json!({"kind":"Status","apiVersion":"v1","status":"Failure","message":"x","reason":reason,"code":code}),
    )
}

fn object_for(path: &str, labels: Value) -> Value {
    let name = path.rsplit('/').next().unwrap_or("");
    if path.contains("/rolebindings/") {
        json!({
            "apiVersion": "rbac.authorization.k8s.io/v1",
            "kind": "RoleBinding",
            "metadata": {"name": name, "namespace": "app-ns", "labels": labels},
            "roleRef": {"apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": name}
        })
    } else {
        json!({
            "apiVersion": "rbac.authorization.k8s.io/v1",
            "kind": "Role",
            "metadata": {"name": name, "namespace": "app-ns", "labels": labels},
            "rules": []
        })
    }
}

fn grant() -> AccessGrant {
    AccessGrant::new(
        "app-ns",
        "g1",
        vec![AccessRule::new(
            vec!["apps".into()],
            vec!["deployments".into()],
            vec!["my-app".into()],
            vec!["get".into()],
        )],
        vec![AccessSubject::new("other-ns", "app-sa")],
    )
}

/// The last two path segments of a request, such as `roles/g1`.
fn tail(path: &str) -> String {
    let mut parts = path.rsplit('/');
    let name = parts.next().unwrap_or("");
    let kind = parts.next().unwrap_or("");
    format!("{kind}/{name}")
}

#[tokio::test]
async fn apply_sends_server_side_apply_for_a_role_and_a_role_binding() {
    let (client, log) = fake_apiserver(|_, path| (200, object_for(path, json!({}))));
    let store = KubernetesAccessStore::new(client);
    let objs = render_access_grants(&[grant()]);

    store
        .apply(AccessObjectKind::Role, "app-ns", "g1", objs[0].clone())
        .await
        .expect("apply role");
    store
        .apply(
            AccessObjectKind::RoleBinding,
            "app-ns",
            "g1",
            objs[1].clone(),
        )
        .await
        .expect("apply rolebinding");

    let log = log.lock().unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].0, "PATCH");
    assert_eq!(log[0].1, format!("{BASE}/app-ns/roles/g1"));
    assert_eq!(log[1].0, "PATCH");
    assert_eq!(log[1].1, format!("{BASE}/app-ns/rolebindings/g1"));
    for entry in log.iter() {
        assert!(
            entry.2.contains("fieldManager=service-k8s-access"),
            "{}",
            entry.2
        );
        assert!(entry.2.contains("force=true"), "{}", entry.2);
    }
    assert_eq!(serde_json::from_str::<Value>(&log[0].3).unwrap(), objs[0]);
    assert_eq!(serde_json::from_str::<Value>(&log[1].3).unwrap(), objs[1]);
}

#[tokio::test]
async fn read_returns_the_labels_and_maps_a_404_to_none() {
    let (client, log) = fake_apiserver(|method, path| {
        if method == "GET" && path.ends_with("/roles/present") {
            (
                200,
                object_for(
                    path,
                    json!({"service-k8s.axiom.dev/owner-identity": "svc-a"}),
                ),
            )
        } else {
            status(404, "NotFound")
        }
    });
    let store = KubernetesAccessStore::new(client);

    let present = store
        .read(AccessObjectKind::Role, "app-ns", "present")
        .await
        .expect("read present role");
    let labels = present.expect("present role exists");
    assert_eq!(
        labels.get("service-k8s.axiom.dev/owner-identity"),
        Some(&"svc-a".to_string())
    );
    assert!(store
        .read(AccessObjectKind::Role, "app-ns", "absent")
        .await
        .expect("read absent role")
        .is_none());
    assert!(store
        .read(AccessObjectKind::RoleBinding, "app-ns", "absent")
        .await
        .expect("read absent rolebinding")
        .is_none());

    let log = log.lock().unwrap();
    assert_eq!(log[0].1, format!("{BASE}/app-ns/roles/present"));
    assert_eq!(log[2].1, format!("{BASE}/app-ns/rolebindings/absent"));
}

#[tokio::test]
async fn delete_treats_404_as_success_and_sends_delete_to_the_namespaced_path() {
    let (client, log) = fake_apiserver(|method, path| {
        if method == "DELETE" && path.ends_with("/roles/gone") {
            status(404, "NotFound")
        } else if method == "DELETE" && path.ends_with("/rolebindings/here") {
            (
                200,
                json!({"kind":"Status","apiVersion":"v1","status":"Success"}),
            )
        } else {
            status(500, "InternalError")
        }
    });
    let store = KubernetesAccessStore::new(client);

    store
        .delete(AccessObjectKind::Role, "app-ns", "gone")
        .await
        .expect("a 404 on delete is success");
    store
        .delete(AccessObjectKind::RoleBinding, "app-ns", "here")
        .await
        .expect("delete rolebinding");

    let log = log.lock().unwrap();
    assert_eq!(log[0].0, "DELETE");
    assert_eq!(log[0].1, format!("{BASE}/app-ns/roles/gone"));
    assert_eq!(log[1].0, "DELETE");
    assert_eq!(log[1].1, format!("{BASE}/app-ns/rolebindings/here"));
}

#[tokio::test]
async fn api_errors_map_to_store_error_kinds() {
    let (client, _log) = fake_apiserver(|method, path| {
        if method == "PATCH" && path.ends_with("/roles/denied") {
            status(403, "Forbidden")
        } else if method == "PATCH" && path.ends_with("/roles/busy") {
            status(409, "Conflict")
        } else {
            status(500, "InternalError")
        }
    });
    let store = KubernetesAccessStore::new(client);

    let denied = store
        .apply(AccessObjectKind::Role, "app-ns", "denied", json!({}))
        .await
        .expect_err("403 is an error");
    assert_eq!(denied.kind, StoreErrorKind::Forbidden);

    let busy = store
        .apply(AccessObjectKind::Role, "app-ns", "busy", json!({}))
        .await
        .expect_err("409 is an error");
    assert_eq!(busy.kind, StoreErrorKind::Conflict);

    let unavailable = store
        .read(AccessObjectKind::Role, "app-ns", "x")
        .await
        .expect_err("500 is an error");
    assert_eq!(unavailable.kind, StoreErrorKind::Unavailable);
}

#[tokio::test]
async fn reconcile_through_the_store_touches_only_roles_and_role_bindings() {
    let (client, log) = fake_apiserver(|method, path| {
        if method == "GET" {
            status(404, "NotFound")
        } else {
            (200, object_for(path, json!({})))
        }
    });
    let store = KubernetesAccessStore::new(client);

    let report = reconcile_access(&store, "svc-a", &[grant()], &[], false).await;
    assert!(report.is_ready);

    let log = log.lock().unwrap();
    let shape: Vec<(String, String)> = log
        .iter()
        .map(|(method, path, _, _)| (method.clone(), tail(path)))
        .collect();
    assert_eq!(
        shape,
        vec![
            ("GET".to_string(), "roles/g1".to_string()),
            ("GET".to_string(), "rolebindings/g1".to_string()),
            ("PATCH".to_string(), "roles/g1".to_string()),
            ("PATCH".to_string(), "rolebindings/g1".to_string()),
        ]
    );
    for (_, path, _, _) in log.iter() {
        assert!(path.starts_with(BASE), "{path}");
        assert!(!path.contains("clusterrole"), "{path}");
        assert!(!path.contains("serviceaccounts"), "{path}");
        assert!(!path.contains("secrets"), "{path}");
        assert!(!path.starts_with("/api/v1/"), "{path}");
    }
}
