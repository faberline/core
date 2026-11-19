use serde_json::json;
use service_k8s::render::{
    owner_ref, ContainerPlan, PodPlan, PodRuntimePolicy, RenderCtx, RestoreJobPlan,
};
use service_k8s::{BackupManifestRef, RestoreJobPolicy, RestoreRequest, RESTORE_FINALIZER};

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

fn manifest() -> BackupManifestRef {
    BackupManifestRef {
        id: "m1".to_string(),
        location: "gs://bucket/m1".to_string(),
    }
}

fn render_job() -> serde_json::Value {
    let pod = PodPlan::new(
        "restore",
        ContainerPlan::new("demo", "demo:1", vec!["restore".into()]),
        PodRuntimePolicy::restricted("sample-restore-identity", json!({})),
    );
    let policy = RestoreJobPolicy {
        backoff_limit: 2,
        active_deadline_seconds: 3600,
        ttl_seconds_after_finished: 600,
    };
    RestoreJobPlan::new("sample-restore-r1", policy, manifest(), pod).render(&context())
}

#[test]
fn job_is_a_batch_v1_job_with_owner_and_labels() {
    let job = render_job();
    assert_eq!(job["kind"], "Job");
    assert_eq!(job["apiVersion"], "batch/v1");
    assert_eq!(job["metadata"]["name"], "sample-restore-r1");
    assert_eq!(job["metadata"]["ownerReferences"][0]["uid"], "uid-1");
    assert_eq!(
        job["metadata"]["labels"]["app.kubernetes.io/component"],
        "restore"
    );
}

#[test]
fn job_sets_bounded_retry_deadline_and_cleanup() {
    let job = render_job();
    assert_eq!(job["spec"]["backoffLimit"], 2);
    assert_eq!(job["spec"]["activeDeadlineSeconds"], 3600);
    assert_eq!(job["spec"]["ttlSecondsAfterFinished"], 600);
    assert_eq!(job["spec"]["template"]["spec"]["restartPolicy"], "Never");
    assert_eq!(
        job["spec"]["template"]["spec"]["serviceAccountName"],
        "sample-restore-identity"
    );
}

#[test]
fn job_carries_the_finalizer() {
    let job = render_job();
    assert_eq!(job["metadata"]["finalizers"], json!([RESTORE_FINALIZER]));
}

#[test]
fn job_takes_the_manifest_by_reference_only() {
    let job = render_job();
    let env = &job["spec"]["template"]["spec"]["containers"][0]["env"];
    assert!(env
        .as_array()
        .unwrap()
        .contains(&json!({"name": "RESTORE_MANIFEST_ID", "value": "m1"})));
    assert!(env
        .as_array()
        .unwrap()
        .contains(&json!({"name": "RESTORE_MANIFEST_LOCATION", "value": "gs://bucket/m1"})));
    let text = job.to_string();
    assert!(!text.contains("\"data\":"));
    assert!(!text.contains("\"binaryData\""));
    assert!(!text.contains("\"stringData\""));
}

#[test]
fn restore_job_is_exported_from_the_crate_root() {
    let request = RestoreRequest {
        restore_id: "r1".to_string(),
        manifest: manifest(),
    };
    assert_eq!(
        service_k8s::plan_restore_job("sample", &request, &[], None),
        Ok(service_k8s::RestoreJobDecision::Start {
            job_name: "sample-restore-r1".to_string()
        })
    );
}
