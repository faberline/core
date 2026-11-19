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
        obj.metadata.deletion_timestamp = Some(Time(chrono::Utc::now()));
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
