use service_k8s::render::{
    owner_ref, BackupCronJobPlan, RenderCtx, WorkloadPlan, WorkloadPlanError,
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
