use service_k8s::render::{
    owner_ref, BackupCronJobPlan, LabelSet, RenderCtx, WorkloadPlan, WorkloadPlanError,
};

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

fn plan() -> BackupCronJobPlan {
    BackupCronJobPlan::new(
        "sample-backup",
        "0 3 * * *",
        "demo:1",
        vec!["demo".into()],
        vec!["backup".into(), "--all".into()],
        "sample-backup-identity",
    )
}

fn render(plan: BackupCronJobPlan) -> serde_json::Value {
    let cx = context();
    let mut workload = WorkloadPlan::new(&cx);
    workload.add_cron_job(plan.into_cron_job().unwrap());
    let mut objects = workload.render().unwrap();
    objects.remove(0)
}

#[test]
fn cron_job_has_name_and_schedule() {
    let cron_job = render(plan());
    assert_eq!(cron_job["kind"], "CronJob");
    assert_eq!(cron_job["apiVersion"], "batch/v1");
    assert_eq!(cron_job["metadata"]["name"], "sample-backup");
    assert_eq!(cron_job["spec"]["schedule"], "0 3 * * *");
}

#[test]
fn cron_job_forbids_concurrent_runs() {
    let cron_job = render(plan());
    assert_eq!(cron_job["spec"]["concurrencyPolicy"], "Forbid");
}

#[test]
fn cron_job_runs_the_app_command_under_the_identity() {
    let cron_job = render(plan());
    let pod = &cron_job["spec"]["jobTemplate"]["spec"]["template"]["spec"];
    assert_eq!(pod["serviceAccountName"], "sample-backup-identity");
    assert_eq!(pod["containers"][0]["image"], "demo:1");
    assert_eq!(pod["containers"][0]["command"], serde_json::json!(["demo"]));
    assert_eq!(
        pod["containers"][0]["args"],
        serde_json::json!(["backup", "--all"])
    );
}

#[test]
fn cron_job_carries_instance_labels() {
    let cron_job = render(plan());
    assert_eq!(
        cron_job["metadata"]["labels"]["app.kubernetes.io/instance"],
        "sample"
    );
    assert_eq!(
        cron_job["metadata"]["labels"]["app.kubernetes.io/component"],
        "backup"
    );
}

#[test]
fn cron_job_carries_owner_reference() {
    let cron_job = render(plan());
    assert_eq!(cron_job["metadata"]["ownerReferences"][0]["uid"], "uid-1");
}

#[test]
fn empty_command_is_refused() {
    let refused =
        BackupCronJobPlan::new("sample-backup", "0 3 * * *", "demo:1", vec![], vec![], "sa")
            .into_cron_job()
            .unwrap_err();
    assert_eq!(
        refused,
        WorkloadPlanError::EmptyCommand("sample-backup".into())
    );
}

fn pod_spec(cron_job: &serde_json::Value) -> &serde_json::Value {
    &cron_job["spec"]["jobTemplate"]["spec"]["template"]["spec"]
}

#[test]
fn cron_job_defaults_are_unchanged_without_builders() {
    let cron_job = render(plan());
    let pod = pod_spec(&cron_job);
    let container = &pod["containers"][0];
    assert!(container.get("imagePullPolicy").is_none());
    assert!(container.get("resources").is_none());
    assert!(container.get("securityContext").is_none());
    assert_eq!(container["env"], serde_json::json!([]));
    assert!(pod.get("nodeSelector").is_none());
    assert!(pod.get("tolerations").is_none());
    assert!(pod.get("volumes").is_none());
    let labels = &cron_job["spec"]["jobTemplate"]["spec"]["template"]["metadata"]["labels"];
    assert!(labels.get("sift.axiom.dev/role").is_none());
}

#[test]
fn cron_job_sets_pull_policy_env_and_resources() {
    let cron_job = render(
        plan()
            .with_image_pull_policy("IfNotPresent")
            .with_env(vec![serde_json::json!({
                "name": "DEMO_BACKUP_TOKEN",
                "valueFrom": {"secretKeyRef": {"name": "admin", "key": "token"}}
            })])
            .with_resources(serde_json::json!({
                "requests": {"cpu": "100m", "memory": "128Mi"},
                "limits": {"cpu": "500m", "memory": "512Mi"}
            })),
    );
    let container = &pod_spec(&cron_job)["containers"][0];
    assert_eq!(container["imagePullPolicy"], "IfNotPresent");
    assert_eq!(container["env"][0]["name"], "DEMO_BACKUP_TOKEN");
    assert_eq!(
        container["env"][0]["valueFrom"]["secretKeyRef"]["key"],
        "token"
    );
    assert_eq!(container["resources"]["limits"]["memory"], "512Mi");
}

#[test]
fn cron_job_sets_volumes_mounts_and_container_security_context() {
    let cron_job = render(
        plan()
            .with_volumes(vec![serde_json::json!({
                "name": "client-token",
                "projected": {"sources": []}
            })])
            .with_volume_mounts(vec![serde_json::json!({
                "name": "client-token",
                "mountPath": "/var/run/secrets/demo/client",
                "readOnly": true
            })])
            .with_container_security_context(serde_json::json!({
                "runAsNonRoot": true,
                "readOnlyRootFilesystem": true
            })),
    );
    let pod = pod_spec(&cron_job);
    assert_eq!(pod["volumes"][0]["name"], "client-token");
    assert_eq!(
        pod["containers"][0]["volumeMounts"][0]["mountPath"],
        "/var/run/secrets/demo/client"
    );
    assert_eq!(
        pod["containers"][0]["securityContext"]["readOnlyRootFilesystem"],
        true
    );
}

#[test]
fn cron_job_sets_placement_and_extra_selector_labels() {
    let cron_job = render(
        plan()
            .with_node_selector(serde_json::json!({"pool": "batch"}))
            .with_tolerations(vec![serde_json::json!({
                "key": "dedicated",
                "operator": "Exists"
            })])
            .with_selector_labels(LabelSet::from([(
                "demo.axiom.dev/role".to_string(),
                "backup".to_string(),
            )])),
    );
    let pod = pod_spec(&cron_job);
    assert_eq!(pod["nodeSelector"]["pool"], "batch");
    assert_eq!(pod["tolerations"][0]["key"], "dedicated");
    let labels = &cron_job["spec"]["jobTemplate"]["spec"]["template"]["metadata"]["labels"];
    assert_eq!(labels["demo.axiom.dev/role"], "backup");
}

#[test]
fn cron_job_with_builders_keeps_fixed_policy() {
    let cron_job = render(
        plan()
            .with_image_pull_policy("IfNotPresent")
            .with_env(vec![serde_json::json!({
                "name": "DEMO_BACKUP_TOKEN",
                "valueFrom": {"secretKeyRef": {"name": "admin", "key": "token"}}
            })])
            .with_resources(serde_json::json!({
                "requests": {"cpu": "100m", "memory": "128Mi"},
                "limits": {"cpu": "500m", "memory": "512Mi"}
            }))
            .with_volumes(vec![serde_json::json!({
                "name": "client-token",
                "projected": {"sources": []}
            })])
            .with_volume_mounts(vec![serde_json::json!({
                "name": "client-token",
                "mountPath": "/var/run/secrets/demo/client",
                "readOnly": true
            })])
            .with_container_security_context(serde_json::json!({
                "runAsNonRoot": true,
                "readOnlyRootFilesystem": true
            }))
            .with_node_selector(serde_json::json!({"pool": "batch"}))
            .with_tolerations(vec![serde_json::json!({
                "key": "dedicated",
                "operator": "Exists"
            })])
            .with_selector_labels(LabelSet::from([(
                "demo.axiom.dev/role".to_string(),
                "backup".to_string(),
            )])),
    );
    assert_eq!(cron_job["spec"]["concurrencyPolicy"], "Forbid");
    let pod = pod_spec(&cron_job);
    assert_eq!(pod["restartPolicy"], "OnFailure");
    assert_eq!(pod["serviceAccountName"], "sample-backup-identity");
    assert_eq!(pod["securityContext"]["runAsNonRoot"], true);
}
