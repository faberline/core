//! Children in another namespace are finalized, pruned and refused when
//! undeclared (#665, the third step of #663).
//!
//! Black box, like `prune_unserved_api_is_scoped.rs`: a fake apiserver answers
//! by route and records each request. Each case drives one pass through the
//! public [`service_k8s::controller::reconcile_once`] and asserts only on what
//! the apiserver saw.

#![cfg(feature = "controller")]

use std::sync::{Arc, Mutex};

use kube::client::Body;
use kube::runtime::controller::Action;
use kube::{Client, CustomResource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use service_k8s::controller::{reconcile_once, Error};
use service_k8s::service::{
    ClusterScopedChild, ManagedService, OtherNamespaceChild, ReadinessTarget, ReadyFacts,
};
use service_k8s::{Election, INSTANCE_UID_LABEL};

const OWNER_NS: &str = "acme";
const OWNER_UID: &str = "u-1";
const MANAGER: &str = "grouped-e2e-operator";
const OTHER_FINALIZER: &str = "service-k8s.axiom.dev/grouped-e2e-operator-other-namespace-children";
const CLUSTER_FINALIZER: &str = "service-k8s.axiom.dev/grouped-e2e-operator-cluster-children";
const CRB: &str = "grouped-crb";

/// One request the fake apiserver answered.
#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    body: Value,
}

type Log = Arc<Mutex<Vec<Recorded>>>;

/// A fake apiserver that answers `route(method, path)` and records every
/// request it was asked, body included.
fn fake_apiserver(
    route: impl Fn(&str, &str) -> (u16, Value) + Send + Sync + 'static,
) -> (Client, Log) {
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let seen = log.clone();
    let route = Arc::new(route);
    let service = tower::service_fn(move |req: http::Request<Body>| {
        let seen = seen.clone();
        let route = route.clone();
        async move {
            let method = req.method().to_string();
            let path = req.uri().path().to_string();
            let bytes = req.into_body().collect_bytes().await.unwrap_or_default();
            let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            let (code, response) = route(&method, &path);
            seen.lock().unwrap().push(Recorded { method, path, body });
            Ok::<_, std::convert::Infallible>(
                http::Response::builder()
                    .status(code)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&response).unwrap()))
                    .unwrap(),
            )
        }
    });
    (Client::new(service, OWNER_NS), log)
}

/// A namespaced placement: one child in one namespace.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
struct Placed {
    namespace: String,
    name: String,
}

#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "service-k8s.e2e",
    version = "v1",
    kind = "GroupedService",
    namespaced
)]
struct GroupedServiceSpec {
    children: Vec<Placed>,
    retired: Vec<Placed>,
    declare: bool,
    cluster: bool,
}

impl ManagedService for GroupedService {
    const MANAGER: &'static str = MANAGER;

    /// One ConfigMap per `children` entry, in the namespace the entry names.
    fn render(&self) -> Vec<Value> {
        self.spec
            .children
            .iter()
            .map(|p| {
                json!({
                    "apiVersion": "v1",
                    "kind": "ConfigMap",
                    "metadata": { "name": p.name, "namespace": p.namespace },
                    "data": { "k": "v" },
                })
            })
            .collect()
    }

    fn readiness_targets(&self) -> Vec<ReadinessTarget> {
        Vec::new()
    }

    fn status_patch(&self, _ready: &ReadyFacts) -> Value {
        json!({ "status": {} })
    }

    /// Declared children (desired) and retired ones (not desired), when
    /// `declare` is set. Nothing when it is not.
    fn other_namespace_children(&self) -> Vec<OtherNamespaceChild> {
        if !self.spec.declare {
            return Vec::new();
        }
        let mut out: Vec<OtherNamespaceChild> = self
            .spec
            .children
            .iter()
            .map(|p| OtherNamespaceChild::new("v1", "ConfigMap", &p.namespace, &p.name, true))
            .collect();
        out.extend(
            self.spec
                .retired
                .iter()
                .map(|p| OtherNamespaceChild::new("v1", "ConfigMap", &p.namespace, &p.name, false)),
        );
        out
    }

    fn cluster_scoped_children(&self) -> Vec<ClusterScopedChild> {
        if !self.spec.cluster {
            return Vec::new();
        }
        vec![ClusterScopedChild::new(
            "rbac.authorization.k8s.io/v1",
            "ClusterRoleBinding",
            CRB,
            true,
        )]
    }
}

/// The subject: an owner in `acme` with UID `u-1`. `deleting` sets a
/// deletion timestamp; `finalizers` are the finalizers the owner already has.
fn subject(
    spec: GroupedServiceSpec,
    deleting: bool,
    finalizers: Vec<String>,
) -> Arc<GroupedService> {
    let mut obj = GroupedService::new("grouped", spec);
    obj.metadata.namespace = Some(OWNER_NS.to_string());
    obj.metadata.uid = Some(OWNER_UID.to_string());
    obj.metadata.finalizers = Some(finalizers);
    if deleting {
        obj.metadata.deletion_timestamp =
            Some(serde_json::from_value(json!("2026-11-27T00:00:00Z")).unwrap());
    }
    Arc::new(obj)
}

/// An election that holds the lease, so the pass is a leader's pass.
fn leader() -> Arc<Election> {
    let election = Election::new("e2e".to_string());
    election
        .is_leader
        .store(true, std::sync::atomic::Ordering::Relaxed);
    election
}

/// The CR body an answer to a CR request carries, so the client can
/// deserialize it.
fn cr_json(spec: Value, finalizers: Value, deleting: bool) -> Value {
    let mut meta = json!({
        "name": "grouped",
        "namespace": OWNER_NS,
        "uid": OWNER_UID,
        "finalizers": finalizers,
    });
    if deleting {
        meta["deletionTimestamp"] = json!("2026-11-27T00:00:00Z");
    }
    json!({
        "apiVersion": "service-k8s.e2e/v1",
        "kind": "GroupedService",
        "metadata": meta,
        "spec": spec,
    })
}

/// A 404 from the resource handler: the API is served and the object is absent.
fn served_but_absent() -> (u16, Value) {
    (
        404,
        json!({ "kind": "Status", "status": "Failure", "message": "not found",
                "reason": "NotFound", "code": 404 }),
    )
}

fn methods_and_paths(log: &Log) -> Vec<String> {
    log.lock()
        .unwrap()
        .iter()
        .map(|r| format!("{} {}", r.method, r.path))
        .collect()
}

/// A plain 200 with an empty object, for every request.
fn ok_everything(_method: &str, _path: &str) -> (u16, Value) {
    (200, json!({}))
}

// 1. An undeclared namespace fails the pass before any write.

#[tokio::test]
async fn a_child_in_an_undeclared_namespace_fails_the_pass_before_any_write() {
    let spec = GroupedServiceSpec {
        children: vec![Placed {
            namespace: "other-a".into(),
            name: "shared".into(),
        }],
        retired: Vec::new(),
        declare: false,
        cluster: false,
    };
    let (client, log) = fake_apiserver(ok_everything);

    let result = reconcile_once(client, subject(spec, false, Vec::new()), leader()).await;

    assert!(
        matches!(result, Err(Error::Plan(_))),
        "an undeclared namespace must refuse the plan, got {result:?}"
    );
    let seen = methods_and_paths(&log);
    assert!(
        seen.iter().all(|entry| !entry.contains("other-a")),
        "no request may touch the undeclared namespace: {seen:?}"
    );
}

// 2. Deleting the owner runs both cleanups with one finalizer patch.

#[tokio::test]
async fn deleting_the_owner_runs_both_cleanups_with_one_finalizer_patch() {
    let spec = GroupedServiceSpec {
        children: vec![Placed {
            namespace: "other-a".into(),
            name: "shared".into(),
        }],
        retired: Vec::new(),
        declare: true,
        cluster: true,
    };
    let finalizers = vec![CLUSTER_FINALIZER.to_string(), OTHER_FINALIZER.to_string()];
    let (client, log) = fake_apiserver(|method, path| match (method, path) {
        ("GET", p) if p.ends_with("/namespaces/other-a/configmaps/shared") => served_but_absent(),
        ("GET", p) if p.ends_with("/clusterrolebindings/grouped-crb") => served_but_absent(),
        ("PATCH", p) if p.ends_with("/groupedservices/grouped") => (
            200,
            cr_json(
                json!({ "children": [], "retired": [], "declare": true, "cluster": true }),
                json!([]),
                true,
            ),
        ),
        _ => (
            500,
            json!({ "kind": "Status", "status": "Failure", "code": 500 }),
        ),
    });

    let action = reconcile_once(client, subject(spec, true, finalizers), leader())
        .await
        .expect("a deleting owner whose children are gone completes");

    let seen = log.lock().unwrap().clone();
    let entries = methods_and_paths(&log);
    assert!(
        entries
            .iter()
            .any(|e| e == "GET /api/v1/namespaces/other-a/configmaps/shared"),
        "the other-namespace child is read: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.ends_with("/clusterrolebindings/grouped-crb")),
        "the cluster child is read: {entries:?}"
    );
    let patches: Vec<&Recorded> = seen
        .iter()
        .filter(|r| r.method == "PATCH" && r.path.ends_with("/groupedservices/grouped"))
        .collect();
    assert_eq!(patches.len(), 1, "exactly one finalizer patch: {entries:?}");
    assert_eq!(
        patches[0].body["metadata"]["finalizers"],
        json!([]),
        "both finalizers are removed in that one patch: {}",
        patches[0].body
    );
    assert_eq!(
        format!("{action:?}"),
        format!("{:?}", Action::await_change()),
        "the owner is released once its finalizers are gone"
    );
}

// 3. A child that is no longer declared is deleted during a normal pass.

#[tokio::test]
async fn a_not_desired_child_is_deleted_during_a_normal_pass() {
    let spec = GroupedServiceSpec {
        children: Vec::new(),
        retired: vec![Placed {
            namespace: "other-a".into(),
            name: "old".into(),
        }],
        declare: true,
        cluster: false,
    };
    let live_old = json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {
            "name": "old",
            "namespace": "other-a",
            "uid": "cm-uid",
            "resourceVersion": "7",
            "labels": { INSTANCE_UID_LABEL: OWNER_UID },
            "managedFields": [{ "manager": MANAGER, "operation": "Apply", "apiVersion": "v1" }],
        },
    });
    let (client, log) = fake_apiserver(move |method, path| match (method, path) {
        ("GET", p) if p.ends_with("/namespaces/other-a/configmaps/old") => (200, live_old.clone()),
        ("DELETE", p) if p.ends_with("/namespaces/other-a/configmaps/old") => (
            200,
            json!({ "kind": "Status", "status": "Success", "code": 200 }),
        ),
        _ => (
            500,
            json!({ "kind": "Status", "status": "Failure", "code": 500 }),
        ),
    });

    let action = reconcile_once(
        client,
        subject(spec, false, vec![OTHER_FINALIZER.to_string()]),
        leader(),
    )
    .await
    .expect("a retired child is removed in a normal pass");

    let seen = methods_and_paths(&log);
    let get = seen
        .iter()
        .position(|e| e == "GET /api/v1/namespaces/other-a/configmaps/old")
        .unwrap_or_else(|| panic!("the retired child was never read: {seen:?}"));
    let delete = seen
        .iter()
        .position(|e| e == "DELETE /api/v1/namespaces/other-a/configmaps/old")
        .unwrap_or_else(|| panic!("the retired child was never deleted: {seen:?}"));
    assert!(get < delete, "read before delete: {seen:?}");
    assert_eq!(
        format!("{action:?}"),
        format!("{:?}", Action::requeue(std::time::Duration::from_secs(1))),
        "a delete requested requeues after 1 s"
    );
}

// 4. A service without other-namespace children installs no new finalizer.

#[tokio::test]
async fn a_service_without_other_children_installs_no_new_finalizer() {
    let spec = GroupedServiceSpec {
        children: vec![Placed {
            namespace: OWNER_NS.into(),
            name: "local".into(),
        }],
        retired: Vec::new(),
        declare: false,
        cluster: false,
    };
    let (client, log) = fake_apiserver(|method, path| match (method, path) {
        ("PATCH", p) if p.ends_with("/namespaces/acme/configmaps/local") => (
            200,
            json!({ "apiVersion": "v1", "kind": "ConfigMap",
                    "metadata": { "name": "local", "namespace": OWNER_NS } }),
        ),
        ("PATCH", p) if p.ends_with("/groupedservices/grouped/status") => (
            200,
            cr_json(
                json!({ "children": [], "retired": [], "declare": false, "cluster": false }),
                json!([]),
                false,
            ),
        ),
        ("POST", p) if p.ends_with("/events") => (201, json!({ "metadata": { "name": "e" } })),
        _ => (
            500,
            json!({ "kind": "Status", "status": "Failure", "code": 500 }),
        ),
    });

    reconcile_once(client, subject(spec, false, Vec::new()), leader())
        .await
        .expect("a same-namespace child is applied as before");

    let finalizer_writes: Vec<Recorded> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.method == "PATCH" && r.path.ends_with("/groupedservices/grouped"))
        .filter(|r| r.body.to_string().contains("other-namespace-children"))
        .cloned()
        .collect();
    assert!(
        finalizer_writes.is_empty(),
        "no other-namespace finalizer is installed: {finalizer_writes:?}"
    );
}
