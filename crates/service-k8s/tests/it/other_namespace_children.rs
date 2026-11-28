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
use kube::{Client, CustomResource, Resource};
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
    namespaced,
    status = "GroupedStatus"
)]
struct GroupedServiceSpec {
    children: Vec<Placed>,
    retired: Vec<Placed>,
    declare: bool,
    cluster: bool,
}

/// The status subresource. The live pass patches it at the end of each pass.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
struct GroupedStatus {
    ready: i64,
}

impl ManagedService for GroupedService {
    const MANAGER: &'static str = MANAGER;

    /// One ConfigMap per `children` entry, in the namespace the entry names. A
    /// child in the owner's own namespace carries the owner reference; a child
    /// in another namespace cannot, so it carries none.
    fn render(&self) -> Vec<Value> {
        let owner = self
            .controller_owner_ref(&())
            .map(|o| serde_json::to_value(o).expect("owner reference"));
        let owner_ns = self.metadata.namespace.clone().unwrap_or_default();
        self.spec
            .children
            .iter()
            .map(|p| {
                let mut meta = json!({ "name": p.name, "namespace": p.namespace });
                if let (Some(owner), true) = (&owner, p.namespace == owner_ns) {
                    meta["ownerReferences"] = json!([owner]);
                }
                json!({
                    "apiVersion": "v1",
                    "kind": "ConfigMap",
                    "metadata": meta,
                    "data": { "k": "v" },
                })
            })
            .collect()
    }

    fn readiness_targets(&self) -> Vec<ReadinessTarget> {
        Vec::new()
    }

    fn status_patch(&self, _ready: &ReadyFacts) -> Value {
        json!({ "status": { "ready": 0 } })
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

/// The live case of #666: apply, prune and cleanup across two namespaces on a
/// kind cluster. CI's kind job runs it with `--include-ignored`. A local run
/// needs a kind cluster and `SERVICE_K8S_LIVE_KUBE=1`.
mod live {
    use std::future::Future;
    use std::sync::Arc;
    use std::time::Duration;

    use k8s_openapi::api::core::v1::{ConfigMap, Namespace};
    use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use kube::api::{Api, DeleteParams, Patch, PatchParams, PostParams};
    use kube::runtime::controller::Action;
    use kube::{Client, CustomResourceExt};
    use serde_json::{json, Value};

    use super::{
        leader, reconcile_once, Error, GroupedService, GroupedServiceSpec, Placed,
        INSTANCE_UID_LABEL, OTHER_FINALIZER,
    };

    const CRD_NAME: &str = "groupedservices.service-k8s.e2e";
    const FIELD_MANAGER: &str = "c663";

    /// Refuse to run against a cluster nobody said to run against.
    ///
    /// `SERVICE_K8S_LIVE_KUBE=1` is consent to a live run, not a destination:
    /// `Config::infer()` reads the ambient kubeconfig, so the flag alone points
    /// these cases at whichever context happens to be selected. They create
    /// namespaces and delete namespaces. So the destination is checked too — a
    /// `kind-` context by default, or exactly what `SERVICE_K8S_LIVE_KUBE_CONTEXT`
    /// names, for a throwaway cluster whose context is called something else.
    fn live_or_skip() {
        let live = std::env::var("SERVICE_K8S_LIVE_KUBE").unwrap_or_default();
        assert_eq!(
            live, "1",
            "SERVICE_K8S_LIVE_KUBE=1 must be set to run a live cluster test"
        );
        let context = kube::config::Kubeconfig::read()
            .expect("read the kubeconfig these cases would otherwise run against")
            .current_context
            .expect(
                "the kubeconfig names no current-context, so there is no way to tell what \
                 cluster this would create and delete namespaces in",
            );
        match std::env::var("SERVICE_K8S_LIVE_KUBE_CONTEXT") {
            Ok(want) => assert_eq!(
                context, want,
                "kubeconfig current-context is `{context}`, but SERVICE_K8S_LIVE_KUBE_CONTEXT \
                 names `{want}`"
            ),
            Err(_) => assert!(
                context.starts_with("kind-"),
                "these cases create and delete namespaces, and kubeconfig current-context is \
                 `{context}` — not a `kind-` cluster. Point kubectl at a throwaway cluster, or \
                 set SERVICE_K8S_LIVE_KUBE_CONTEXT={context} to say that is deliberate"
            ),
        }
    }

    /// Run `body`, then delete every namespace in `names`, even when it panicked.
    async fn with_teardown<F>(admin: &Client, names: &[String], body: F)
    where
        F: Future<Output = ()>,
    {
        use futures::FutureExt as _;
        let outcome = std::panic::AssertUnwindSafe(body).catch_unwind().await;
        for ns in names {
            let _ = Api::<Namespace>::all(admin.clone())
                .delete(ns, &DeleteParams::default())
                .await;
        }
        if let Err(panic) = outcome {
            // Re-raise the original panic, so the test fails with its own message.
            std::panic::resume_unwind(panic);
        }
    }

    /// Poll the CRD until its `Established` condition is `True`, at most 30 s.
    async fn wait_established(crds: &Api<CustomResourceDefinition>) {
        for _ in 0..60 {
            let crd = crds.get(CRD_NAME).await.expect("read the CRD");
            let established = crd
                .status
                .and_then(|s| s.conditions)
                .unwrap_or_default()
                .iter()
                .any(|c| c.type_ == "Established" && c.status == "True");
            if established {
                return;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        panic!("the CRD {CRD_NAME} is not Established after 30 s");
    }

    /// One pass on the CR as the apiserver has it now.
    async fn pass(client: &Client, gs: &Api<GroupedService>) -> Result<Action, Error> {
        let cr = gs.get("grouped").await.expect("read the CR");
        reconcile_once(client.clone(), Arc::new(cr), leader()).await
    }

    async fn configmap(client: &Client, ns: &str, name: &str) -> Option<ConfigMap> {
        Api::<ConfigMap>::namespaced(client.clone(), ns)
            .get_opt(name)
            .await
            .expect("read the ConfigMap")
    }

    async fn patch_spec(gs: &Api<GroupedService>, spec: Value) {
        gs.patch(
            "grouped",
            &PatchParams::default(),
            &Patch::Merge(json!({ "spec": spec })),
        )
        .await
        .expect("patch the CR spec");
    }

    async fn exercise(client: Client, owner: String, a: String, b: String) {
        for ns in [&owner, &a, &b] {
            Api::<Namespace>::all(client.clone())
                .create(
                    &PostParams::default(),
                    &Namespace {
                        metadata: ObjectMeta {
                            name: Some(ns.clone()),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )
                .await
                .expect("create namespace");
        }

        // a. Apply the CRD by server-side apply, and wait until it is served.
        let crds: Api<CustomResourceDefinition> = Api::all(client.clone());
        crds.patch(
            CRD_NAME,
            &PatchParams::apply(FIELD_MANAGER).force(),
            &Patch::Apply(&GroupedService::crd()),
        )
        .await
        .expect("apply the CRD");
        wait_established(&crds).await;

        // b. The CR in the owner namespace, with one child in each namespace.
        let gs: Api<GroupedService> = Api::namespaced(client.clone(), &owner);
        let spec = GroupedServiceSpec {
            children: vec![
                Placed {
                    namespace: a.clone(),
                    name: "one".into(),
                },
                Placed {
                    namespace: b.clone(),
                    name: "two".into(),
                },
                Placed {
                    namespace: owner.clone(),
                    name: "home".into(),
                },
            ],
            retired: Vec::new(),
            declare: true,
            cluster: false,
        };
        gs.create(
            &PostParams::default(),
            &GroupedService::new("grouped", spec),
        )
        .await
        .expect("create the CR");

        // c. Up to 5 passes. The first installs the finalizer; later passes apply.
        let mut last = None;
        for _ in 0..5 {
            last = Some(pass(&client, &gs).await);
            if configmap(&client, &a, "one").await.is_some()
                && configmap(&client, &b, "two").await.is_some()
                && configmap(&client, &owner, "home").await.is_some()
            {
                break;
            }
        }
        last.expect("a pass ran")
            .expect("the pass that applies the children succeeds");

        let cr = gs.get("grouped").await.expect("read the CR");
        let uid = cr.metadata.uid.clone().expect("the CR has a UID");
        assert!(
            cr.metadata
                .finalizers
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|f| f == OTHER_FINALIZER),
            "the CR carries the other-namespace finalizer: {:?}",
            cr.metadata.finalizers
        );
        let one = configmap(&client, &a, "one")
            .await
            .expect("one exists in a");
        assert_eq!(
            one.metadata
                .labels
                .as_ref()
                .and_then(|l| l.get(INSTANCE_UID_LABEL))
                .map(String::as_str),
            Some(uid.as_str()),
            "the child in a carries the owner UID label"
        );
        assert!(
            one.metadata
                .owner_references
                .as_deref()
                .unwrap_or_default()
                .is_empty(),
            "the child in a has no owner reference"
        );
        let home = configmap(&client, &owner, "home")
            .await
            .expect("home exists in the owner namespace");
        let refs = home.metadata.owner_references.clone().unwrap_or_default();
        assert_eq!(refs.len(), 1, "home keeps one owner reference: {refs:?}");
        assert_eq!(refs[0].uid, uid, "the owner reference names the CR");

        // d. Prune: `one` is retired and goes; `two` stays.
        patch_spec(
            &gs,
            json!({
                "children": [
                    { "namespace": b, "name": "two" },
                    { "namespace": owner, "name": "home" },
                ],
                "retired": [{ "namespace": a, "name": "one" }],
            }),
        )
        .await;
        let mut last = None;
        for _ in 0..5 {
            last = Some(pass(&client, &gs).await);
            if configmap(&client, &a, "one").await.is_none() {
                break;
            }
        }
        assert!(
            configmap(&client, &a, "one").await.is_none(),
            "the retired child is pruned; last pass: {last:?}"
        );
        assert!(
            configmap(&client, &b, "two").await.is_some(),
            "the declared child in b stays"
        );

        // e. Guard: a ConfigMap without our labels is refused and kept.
        Api::<ConfigMap>::namespaced(client.clone(), &b)
            .create(
                &PostParams::default(),
                &ConfigMap {
                    metadata: ObjectMeta {
                        name: Some("foreign".into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .await
            .expect("create the foreign ConfigMap");
        patch_spec(
            &gs,
            json!({ "retired": [{ "namespace": b, "name": "foreign" }] }),
        )
        .await;
        let refused = pass(&client, &gs).await;
        assert!(
            matches!(refused, Err(Error::NamespacedOwnership { .. })),
            "the guard refuses a foreign ConfigMap: {refused:?}"
        );
        assert!(
            configmap(&client, &b, "foreign").await.is_some(),
            "the foreign ConfigMap is kept"
        );
        patch_spec(&gs, json!({ "retired": [] })).await;

        // f. Cleanup: deleting the owner removes its child in b, then the CR.
        gs.delete("grouped", &DeleteParams::default())
            .await
            .expect("delete the CR");
        let deleting = gs
            .get("grouped")
            .await
            .expect("the CR waits on its finalizers");
        assert!(
            deleting.metadata.deletion_timestamp.is_some(),
            "the CR is being deleted"
        );
        let mut last = None;
        for _ in 0..3 {
            let Some(cr) = gs.get_opt("grouped").await.expect("read the CR") else {
                break;
            };
            last = Some(reconcile_once(client.clone(), Arc::new(cr), leader()).await);
        }
        assert!(
            configmap(&client, &b, "two").await.is_none(),
            "the child in b is deleted with its owner; last pass: {last:?}"
        );
        assert!(
            gs.get_opt("grouped").await.expect("read the CR").is_none(),
            "the CR is gone after the cleanup; last pass: {last:?}"
        );
    }

    #[tokio::test]
    #[ignore = "live kind cluster: needs SERVICE_K8S_LIVE_KUBE=1; runs in CI job `e2e: service-k8s kind`"]
    async fn kind_applies_prunes_and_cleans_up_children_in_two_namespaces() {
        live_or_skip();
        let client = Client::try_default().await.expect("admin client");
        let pid = std::process::id();
        let owner = format!("c663-owner-{pid}");
        let a = format!("c663-a-{pid}");
        let b = format!("c663-b-{pid}");
        let names = [owner.clone(), a.clone(), b.clone()];
        with_teardown(&client, &names, exercise(client.clone(), owner, a, b)).await;
    }
}
