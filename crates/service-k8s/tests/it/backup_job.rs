use serde_json::json;
use service_k8s::render::{
    owner_ref, BackupJobPlan, ContainerPlan, PodPlan, PodRuntimePolicy, RenderCtx,
};
use service_k8s::BackupJobPolicy;

fn context() -> RenderCtx<'static> {
    RenderCtx::new(
        "demo",
        "demo-operator",
        "demo.axiom.dev/v1",
        "Demo",
        "sample",
        "observability",
    )
    .with_owner(owner_ref("demo.axiom.dev/v1", "Demo", "sample", "uid-1"))
}

fn render_job() -> serde_json::Value {
    let pod = PodPlan::new(
        "backup",
        ContainerPlan::new("demo", "demo:1", vec!["backup".into()]),
        PodRuntimePolicy::restricted("sample-backup-identity", json!({})),
    );
    let policy = BackupJobPolicy {
        backoff_limit: 2,
        active_deadline_seconds: 3600,
        ttl_seconds_after_finished: 600,
    };
    BackupJobPlan::new("sample-backup-s1", policy, pod).render(&context())
}

#[test]
fn job_is_a_batch_v1_job_without_a_schedule() {
    let job = render_job();
    assert_eq!(job["kind"], "Job");
    assert_eq!(job["apiVersion"], "batch/v1");
    assert_eq!(job["metadata"]["name"], "sample-backup-s1");
    assert!(job["spec"].get("schedule").is_none());
}

#[test]
fn job_carries_owner_reference_and_instance_labels() {
    let job = render_job();
    assert_eq!(job["metadata"]["ownerReferences"][0]["uid"], "uid-1");
    assert_eq!(
        job["metadata"]["labels"]["app.kubernetes.io/instance"],
        "sample"
    );
    assert_eq!(
        job["metadata"]["labels"]["app.kubernetes.io/component"],
        "backup"
    );
}

#[test]
fn job_sets_retry_deadline_and_cleanup_from_the_policy() {
    let job = render_job();
    assert_eq!(job["spec"]["backoffLimit"], 2);
    assert_eq!(job["spec"]["activeDeadlineSeconds"], 3600);
    assert_eq!(job["spec"]["ttlSecondsAfterFinished"], 600);
}

#[test]
fn job_pod_never_restarts_and_uses_the_given_identity() {
    let job = render_job();
    assert_eq!(job["spec"]["template"]["spec"]["restartPolicy"], "Never");
    assert_eq!(
        job["spec"]["template"]["spec"]["serviceAccountName"],
        "sample-backup-identity"
    );
}
