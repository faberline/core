use super::reconcile_metrics::{counted_ctx, counted_obj, CountedService};
use super::*;
use crate::domain::retention::{DataRetention, RetentionPlan, INSTANCE_UID_LABEL};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use std::collections::BTreeMap;

#[derive(
    kube::CustomResource, Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
#[kube(
    group = "service-k8s.test",
    version = "v1",
    kind = "RetainedService",
    namespaced
)]
struct RetainedServiceSpec {
    replicas: u32,
}

impl ManagedService for RetainedService {
    const MANAGER: &'static str = "retained-operator";

    fn render(&self) -> Vec<Value> {
        vec![]
    }

    fn readiness_targets(&self) -> Vec<service::ReadinessTarget> {
        Vec::new()
    }

    fn status_patch(&self, _ready: &ReadyFacts) -> Value {
        json!({ "status": {} })
    }

    fn data_retention(&self) -> Option<RetentionPlan> {
        Some(RetentionPlan {
            policy: DataRetention::Retain,
            workloads: vec![("StatefulSet", "search".to_string())],
            pvc_labels: BTreeMap::from([(INSTANCE_UID_LABEL.to_string(), "uid-1".to_string())]),
        })
    }
}

const STS: &str = "/apis/apps/v1/namespaces/acme/statefulsets/search";
const CR: &str = "/apis/service-k8s.test/v1/namespaces/acme/retainedservices/search";
const FINALIZER: &str = "service-k8s.axiom.dev/retained-operator-data-retention";

fn retained_ctx(client: Client) -> Arc<Ctx> {
    let election = Election::new("test-identity".to_string());
    election.is_leader.store(true, Ordering::Relaxed);
    Arc::new(Ctx::new::<RetainedService>(
        client,
        Leadership::held(election),
        Arc::new(ControllerMetrics::new(RetainedService::MANAGER)),
    ))
}

fn retained_obj(deleting: bool, finalized: bool) -> Arc<RetainedService> {
    let mut obj = RetainedService::new("search", RetainedServiceSpec { replicas: 1 });
    obj.metadata.namespace = Some("acme".to_string());
    if deleting {
        obj.metadata.deletion_timestamp = Some(Time(k8s_openapi::jiff::Timestamp::now()));
    }
    if finalized {
        obj.metadata.finalizers = Some(vec![FINALIZER.to_string()]);
    }
    Arc::new(obj)
}

fn sts(ready: i64) -> Value {
    json!({
        "apiVersion": "apps/v1",
        "kind": "StatefulSet",
        "metadata": { "name": "search" },
        "status": { "readyReplicas": ready },
    })
}

fn cr_json() -> Value {
    json!({
        "apiVersion": "service-k8s.test/v1",
        "kind": "RetainedService",
        "metadata": { "name": "search", "namespace": "acme" },
        "spec": { "replicas": 1 },
    })
}

#[test]
fn default_retention_is_retain() {
    assert_eq!(DataRetention::default(), DataRetention::Retain);
}

#[tokio::test]
async fn retain_deletion_makes_no_pvc_request() {
    let (client, seen) = fake_apiserver(vec![(200, sts(0)), (200, sts(0)), (200, cr_json())]);
    let ctx = retained_ctx(client);

    let action = reconcile_entry::<RetainedService>(retained_obj(true, true), ctx)
        .await
        .expect("a stopped workload lets the finalizer go");

    assert_eq!(action, Action::await_change());
    let seen = seen.lock().unwrap().clone();
    assert_eq!(
        seen[..3],
        [
            format!("PATCH {STS}"),
            format!("GET {STS}"),
            format!("PATCH {CR}"),
        ]
    );
    assert!(
        seen.iter().all(|r| !r.contains("persistentvolumeclaims")),
        "Retain must not read or delete a PVC: {seen:?}"
    );
}

#[tokio::test]
async fn deletion_scales_the_workload_down_before_the_finalizer_goes() {
    // (a) a pod is still ready: the finalizer stays and the pass requeues.
    let (client, seen) = fake_apiserver(vec![(200, sts(0)), (200, sts(1))]);
    let action = reconcile_entry::<RetainedService>(retained_obj(true, true), retained_ctx(client))
        .await
        .expect("a pass with a ready pod succeeds");
    assert_eq!(action, Action::requeue(Duration::from_secs(1)));
    assert!(
        !seen
            .lock()
            .unwrap()
            .iter()
            .any(|r| r == &format!("PATCH {CR}")),
        "the finalizer must stay while a pod is ready"
    );

    // (b) no pod is ready: the finalizer is removed.
    let (client, seen) = fake_apiserver(vec![(200, sts(0)), (200, sts(0)), (200, cr_json())]);
    let action = reconcile_entry::<RetainedService>(retained_obj(true, true), retained_ctx(client))
        .await
        .expect("a pass with no ready pod succeeds");
    assert_eq!(action, Action::await_change());
    assert!(
        seen.lock()
            .unwrap()
            .iter()
            .any(|r| r == &format!("PATCH {CR}")),
        "the finalizer must be removed once no pod is ready"
    );
}

#[tokio::test]
async fn service_without_a_retention_plan_is_unchanged() {
    let (client, seen) = fake_apiserver(vec![(
        200,
        json!({ "apiVersion": "apps/v1", "kind": "Deployment", "metadata": { "name": "counted" } }),
    )]);
    let ctx = counted_ctx(client, true);

    let _ = reconcile_entry::<CountedService>(counted_obj(), ctx).await;

    let finalizer_patch = "PATCH /apis/service-k8s.test/v1/namespaces/acme/countedservices/counted";
    assert!(
        !seen.lock().unwrap().iter().any(|r| r == finalizer_patch),
        "a service with no plan must not get a retention finalizer"
    );
}

const PVCS: &str = "/api/v1/namespaces/acme/persistentvolumeclaims";
const POLICY_CR: &str = "/apis/service-k8s.test/v1/namespaces/acme/policyservices/search";
const POLICY_FINALIZER: &str = "service-k8s.axiom.dev/policy-operator-data-retention";

#[derive(
    kube::CustomResource, Clone, Debug, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
#[kube(
    group = "service-k8s.test",
    version = "v1",
    kind = "PolicyService",
    namespaced
)]
struct PolicyServiceSpec {
    policy: DataRetention,
}

impl ManagedService for PolicyService {
    const MANAGER: &'static str = "policy-operator";

    fn render(&self) -> Vec<Value> {
        vec![]
    }

    fn readiness_targets(&self) -> Vec<service::ReadinessTarget> {
        Vec::new()
    }

    fn status_patch(&self, _ready: &ReadyFacts) -> Value {
        json!({ "status": {} })
    }

    fn data_retention(&self) -> Option<RetentionPlan> {
        Some(RetentionPlan {
            policy: self.spec.policy,
            workloads: vec![("StatefulSet", "search".to_string())],
            pvc_labels: BTreeMap::from([("app".to_string(), "search".to_string())]),
        })
    }
}

fn policy_ctx(client: Client) -> Arc<Ctx> {
    let election = Election::new("test-identity".to_string());
    election.is_leader.store(true, Ordering::Relaxed);
    Arc::new(Ctx::new::<PolicyService>(
        client,
        Leadership::held(election),
        Arc::new(ControllerMetrics::new(PolicyService::MANAGER)),
    ))
}

fn policy_obj(policy: DataRetention) -> Arc<PolicyService> {
    let mut obj = PolicyService::new("search", PolicyServiceSpec { policy });
    obj.metadata.namespace = Some("acme".to_string());
    obj.metadata.uid = Some("uid-1".to_string());
    obj.metadata.deletion_timestamp = Some(Time(k8s_openapi::jiff::Timestamp::now()));
    obj.metadata.finalizers = Some(vec![POLICY_FINALIZER.to_string()]);
    Arc::new(obj)
}

fn pvc(name: &str, uid_label: &str) -> Value {
    let mut labels = json!({ "app": "search" });
    labels[INSTANCE_UID_LABEL] = json!(uid_label);
    json!({
        "apiVersion": "v1",
        "kind": "PersistentVolumeClaim",
        "metadata": {
            "name": name,
            "namespace": "acme",
            "uid": format!("pvc-{name}"),
            "resourceVersion": "7",
            "labels": labels,
        },
    })
}

fn pvc_list(items: Vec<Value>) -> Value {
    json!({
        "apiVersion": "v1",
        "kind": "PersistentVolumeClaimList",
        "metadata": {},
        "items": items,
    })
}

fn policy_cr() -> Value {
    json!({
        "apiVersion": "service-k8s.test/v1",
        "kind": "PolicyService",
        "metadata": { "name": "search", "namespace": "acme" },
        "spec": { "policy": "Delete" },
    })
}

fn recorded_deletes(seen: &[String]) -> Vec<String> {
    seen.iter()
        .filter(|r| r.starts_with("DELETE"))
        .cloned()
        .collect()
}

#[tokio::test]
async fn delete_removes_only_pvcs_with_this_instance_uid() {
    let (client, seen) = fake_apiserver(vec![
        (200, sts(0)),
        (200, sts(0)),
        (200, pvc_list(vec![pvc("a", "uid-1"), pvc("b", "uid-2")])),
        (200, pvc("a", "uid-1")),
        (200, policy_cr()),
    ]);

    let action =
        reconcile_entry::<PolicyService>(policy_obj(DataRetention::Delete), policy_ctx(client))
            .await
            .expect("a pass with no ready pod succeeds");

    assert_eq!(action, Action::await_change());
    let seen = seen.lock().unwrap().clone();
    assert_eq!(
        recorded_deletes(&seen),
        vec![format!("DELETE {PVCS}/a")],
        "only the PVC of this instance may be deleted: {seen:?}"
    );
}

#[tokio::test]
async fn a_new_instance_with_the_same_name_keeps_its_pvc() {
    // Same name and selector label, but the UID label holds another instance's UID.
    let (client, seen) = fake_apiserver(vec![
        (200, sts(0)),
        (200, sts(0)),
        (200, pvc_list(vec![pvc("a", "uid-old")])),
        (200, policy_cr()),
    ]);

    let action =
        reconcile_entry::<PolicyService>(policy_obj(DataRetention::Delete), policy_ctx(client))
            .await
            .expect("a pass with no ready pod succeeds");

    assert_eq!(action, Action::await_change());
    let seen = seen.lock().unwrap().clone();
    assert!(
        recorded_deletes(&seen).is_empty(),
        "a PVC of another instance must not be deleted: {seen:?}"
    );
}

#[tokio::test]
async fn operator_restart_in_the_middle_of_a_delete_converges() {
    // Pass 1: the first PVC is deleted, the second delete fails with a 500.
    let (client, seen) = fake_apiserver(vec![
        (200, sts(0)),
        (200, sts(0)),
        (200, pvc_list(vec![pvc("a", "uid-1"), pvc("b", "uid-1")])),
        (200, pvc("a", "uid-1")),
        (
            500,
            json!({ "kind": "Status", "status": "Failure", "code": 500 }),
        ),
    ]);

    let result =
        reconcile_entry::<PolicyService>(policy_obj(DataRetention::Delete), policy_ctx(client))
            .await;

    assert!(result.is_err(), "a failed delete must fail the pass");
    let seen = seen.lock().unwrap().clone();
    assert!(
        seen.iter().all(|r| r != &format!("PATCH {POLICY_CR}")),
        "the finalizer must stay while a PVC is left: {seen:?}"
    );

    // Pass 2 after the restart: only `b` is left and it is deleted.
    let (client, seen) = fake_apiserver(vec![
        (200, sts(0)),
        (200, sts(0)),
        (200, pvc_list(vec![pvc("b", "uid-1")])),
        (200, pvc("b", "uid-1")),
        (200, policy_cr()),
    ]);

    let action =
        reconcile_entry::<PolicyService>(policy_obj(DataRetention::Delete), policy_ctx(client))
            .await
            .expect("the second pass converges");

    assert_eq!(action, Action::await_change());
    let seen = seen.lock().unwrap().clone();
    assert!(seen.contains(&format!("DELETE {PVCS}/b")), "{seen:?}");
    assert!(seen.contains(&format!("PATCH {POLICY_CR}")), "{seen:?}");
}

#[tokio::test]
async fn a_pvc_that_is_already_gone_is_success() {
    let (client, seen) = fake_apiserver(vec![
        (200, sts(0)),
        (200, sts(0)),
        (200, pvc_list(vec![pvc("a", "uid-1")])),
        not_found(),
        (200, policy_cr()),
    ]);

    let action =
        reconcile_entry::<PolicyService>(policy_obj(DataRetention::Delete), policy_ctx(client))
            .await
            .expect("a PVC that is already gone is not an error");

    assert_eq!(action, Action::await_change());
    let seen = seen.lock().unwrap().clone();
    assert!(
        seen.contains(&format!("PATCH {POLICY_CR}")),
        "the finalizer must be removed: {seen:?}"
    );
}

#[tokio::test]
async fn a_pvc_that_is_already_terminating_requeues() {
    let mut terminating = pvc("a", "uid-1");
    terminating["metadata"]["deletionTimestamp"] = json!("2026-01-01T00:00:00Z");
    let (client, seen) = fake_apiserver(vec![
        (200, sts(0)),
        (200, sts(0)),
        (200, pvc_list(vec![terminating])),
    ]);

    let action =
        reconcile_entry::<PolicyService>(policy_obj(DataRetention::Delete), policy_ctx(client))
            .await
            .expect("a terminating PVC is waited for, not an error");

    assert_eq!(action, Action::requeue(Duration::from_secs(1)));
    let seen = seen.lock().unwrap().clone();
    assert!(recorded_deletes(&seen).is_empty(), "{seen:?}");
    assert!(
        seen.iter().all(|r| r != &format!("PATCH {POLICY_CR}")),
        "the finalizer must stay while a PVC is terminating: {seen:?}"
    );
}

#[tokio::test]
async fn policy_changed_to_retain_during_deletion_stops_deleting() {
    let (client, seen) = fake_apiserver(vec![(200, sts(0)), (200, sts(0)), (200, policy_cr())]);

    let action =
        reconcile_entry::<PolicyService>(policy_obj(DataRetention::Retain), policy_ctx(client))
            .await
            .expect("a Retain pass with no ready pod succeeds");

    assert_eq!(action, Action::await_change());
    let seen = seen.lock().unwrap().clone();
    assert!(
        seen.contains(&format!("PATCH {POLICY_CR}")),
        "the finalizer must be removed: {seen:?}"
    );
    assert!(
        seen.iter().all(|r| !r.contains("persistentvolumeclaims")),
        "Retain must not read or delete a PVC: {seen:?}"
    );
}
