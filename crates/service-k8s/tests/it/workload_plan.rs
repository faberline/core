use std::collections::BTreeMap;

use serde_json::json;
use service_k8s::render::{
    ClusterRoleBindingPlan, ContainerPlan, CronJobPlan, DeploymentPlan, FqdnMatchPlan,
    FqdnNetworkPolicyPlan, NetworkPeerPlan, NetworkPolicyPlan, NetworkPortPlan, NetworkRulePlan,
    PodPlan, PodRuntimePolicy, RbacRulePlan, RenderCtx, RoleBindingPlan, RolePlan,
    ServiceAccountSubjectPlan, ServicePlan, ServicePortPlan, WorkloadPlan, WorkloadPlanError,
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
}

#[test]
fn one_typed_plan_renders_service_deployment_and_default_deny_network_policy() {
    let cx = context();
    let selector = BTreeMap::from([("demo.axiom.dev/role".to_string(), "query".to_string())]);
    let runtime = PodRuntimePolicy::restricted("sample", json!({}));
    let pod = PodPlan::new(
        "query",
        ContainerPlan::new("demo", "demo:1", vec!["serve".into()]),
        runtime,
    )
    .with_selector_labels(selector.clone());

    let mut plan = WorkloadPlan::new(&cx);
    plan.add_service(ServicePlan::cluster_ip(
        "sample-query",
        "query",
        selector.clone(),
        vec![ServicePortPlan::tcp("http", 7380, "http")],
    ));
    plan.add_deployment(DeploymentPlan::new("sample-query", 1, pod));
    plan.add_network_policy(
        NetworkPolicyPlan::new("sample-query", "query", selector).with_ingress(
            NetworkRulePlan::new(
                vec![NetworkPeerPlan::SameNamespace],
                vec![NetworkPortPlan::tcp(7380)],
            ),
        ),
    );

    let objects = plan.render().expect("render typed workload plan");
    assert_eq!(objects[0]["kind"], "Service");
    assert_eq!(objects[1]["kind"], "Deployment");
    assert_eq!(
        objects[1]["spec"]["template"]["spec"]["securityContext"]["fsGroupChangePolicy"],
        "OnRootMismatch"
    );
    assert_eq!(objects[2]["kind"], "NetworkPolicy");
    assert_eq!(
        objects[2]["spec"]["policyTypes"],
        json!(["Ingress", "Egress"])
    );
    assert_eq!(objects[2]["spec"]["egress"], json!([]));
}

#[test]
fn typed_plan_renders_a_cluster_scoped_binding_without_a_namespaced_owner() {
    let cx = context();
    let mut plan = WorkloadPlan::new(&cx);
    plan.add_cluster_role_binding(
        ClusterRoleBindingPlan::new(
            "demo.observability.sample.auth-delegator",
            "auth-delegation",
            "system:auth-delegator",
        )
        .with_service_account(ServiceAccountSubjectPlan::new("observability", "sample"))
        .with_label("demo.axiom.dev/owner-namespace", "observability"),
    );

    let objects = plan.render().expect("render typed workload plan");
    assert_eq!(objects.len(), 1);
    let binding = &objects[0];
    assert_eq!(binding["kind"], "ClusterRoleBinding");
    assert!(binding["metadata"].get("namespace").is_none());
    assert!(binding["metadata"].get("ownerReferences").is_none());
    assert_eq!(
        binding["metadata"]["labels"]["demo.axiom.dev/owner-namespace"],
        "observability"
    );
    assert_eq!(binding["roleRef"]["name"], "system:auth-delegator");
    assert_eq!(
        binding["subjects"],
        json!([{
            "kind": "ServiceAccount",
            "name": "sample",
            "namespace": "observability"
        }])
    );
}

#[test]
fn typed_plan_limits_external_egress_by_ip_and_gke_fqdn() {
    let cx = context();
    let selector = BTreeMap::from([("demo.axiom.dev/role".to_string(), "store".to_string())]);
    let mut plan = WorkloadPlan::new(&cx);
    plan.add_network_policy(
        NetworkPolicyPlan::new("sample-store", "store", selector.clone()).with_egress(
            NetworkRulePlan::new(
                vec![NetworkPeerPlan::ip_block("169.254.169.254/32")],
                vec![NetworkPortPlan::tcp(80)],
            ),
        ),
    );
    plan.add_fqdn_network_policy(
        FqdnNetworkPolicyPlan::new("sample-store-google-apis", "store", selector)
            .with_match(FqdnMatchPlan::name("storage.googleapis.com"))
            .with_port(NetworkPortPlan::tcp(443)),
    );

    let objects = plan.render().expect("render typed workload plan");
    assert_eq!(
        objects[0]["spec"]["egress"][0]["to"][0]["ipBlock"]["cidr"],
        "169.254.169.254/32"
    );
    assert_eq!(objects[1]["apiVersion"], "networking.gke.io/v1alpha1");
    assert_eq!(objects[1]["kind"], "FQDNNetworkPolicy");
    assert_eq!(
        objects[1]["spec"]["egress"][0]["matches"],
        json!([{"name":"storage.googleapis.com"}])
    );
    assert_eq!(
        objects[1]["spec"]["egress"][0]["ports"],
        json!([{"protocol":"TCP", "port":443}])
    );
}

#[test]
fn any_network_peer_omits_the_direction_selector() {
    let cx = context();
    let selector = BTreeMap::from([("demo.axiom.dev/role".to_string(), "gateway".to_string())]);
    let mut plan = WorkloadPlan::new(&cx);
    plan.add_network_policy(
        NetworkPolicyPlan::new("sample-gateway", "gateway", selector).with_ingress(
            NetworkRulePlan::new(
                vec![NetworkPeerPlan::any()],
                vec![NetworkPortPlan::tcp(7380)],
            ),
        ),
    );

    let objects = plan.render().expect("render typed workload plan");
    let ingress = &objects[0]["spec"]["ingress"][0];
    assert!(
        ingress.get("from").is_none(),
        "an unrestricted peer is expressed by omitting `from`, not by an invalid empty peer"
    );
    assert_eq!(ingress["ports"], json!([{"protocol":"TCP", "port":7380}]));
}

#[test]
fn typed_plan_renders_role_role_binding_and_cron_job_from_constructors() {
    let cx = context();
    let role = RolePlan::new("sample-agent", "auth").with_rule(
        RbacRulePlan::new(
            vec!["demo.axiom.dev".into()],
            vec!["projects".into()],
            vec!["get".into(), "update".into()],
        )
        .with_resource_names(vec!["sample".into()]),
    );
    assert_eq!(role.name(), "sample-agent");
    assert_eq!(role.rules()[0].resource_names(), ["sample"]);
    let binding = RoleBindingPlan::new("sample-agent", "auth", "sample-agent")
        .with_service_account(ServiceAccountSubjectPlan::new("observability", "sample"))
        .with_service_account(ServiceAccountSubjectPlan::new(
            "observability",
            "sample-backup",
        ));
    assert_eq!(binding.subjects()[1].name(), "sample-backup");
    assert_eq!(binding.subjects()[1].namespace(), "observability");
    let pod = PodPlan::new(
        "backup",
        ContainerPlan::new("backup", "demo:1", vec!["backup".into()]),
        PodRuntimePolicy::restricted("sample-backup", json!({})),
    );
    let defaults = CronJobPlan::new("sample-backup", "0 3 * * *", pod.clone());
    assert_eq!(
        (
            defaults.successful_jobs_history_limit(),
            defaults.failed_jobs_history_limit()
        ),
        (3, 1)
    );
    let cron_job = CronJobPlan::new("sample-backup", "0 3 * * *", pod)
        .with_successful_jobs_history_limit(5)
        .with_failed_jobs_history_limit(2);

    let mut plan = WorkloadPlan::new(&cx);
    plan.add_role(role);
    plan.add_role_binding(binding);
    plan.add_cron_job(cron_job);
    let objects = plan.render().expect("render typed workload plan");

    assert_eq!(
        objects[0]["rules"],
        json!([{
            "apiGroups": ["demo.axiom.dev"],
            "resources": ["projects"],
            "resourceNames": ["sample"],
            "verbs": ["get", "update"]
        }])
    );
    assert_eq!(objects[1]["roleRef"]["name"], "sample-agent");
    assert_eq!(
        objects[1]["subjects"],
        json!([
            {"kind": "ServiceAccount", "name": "sample", "namespace": "observability"},
            {"kind": "ServiceAccount", "name": "sample-backup", "namespace": "observability"}
        ])
    );
    let spec = &objects[2]["spec"];
    assert_eq!(spec["schedule"], "0 3 * * *");
    assert_eq!(spec["successfulJobsHistoryLimit"], 5);
    assert_eq!(spec["failedJobsHistoryLimit"], 2);
    assert_eq!(
        spec["jobTemplate"]["spec"]["template"]["spec"]["restartPolicy"],
        "OnFailure"
    );
}

#[test]
fn typed_plan_renders_cron_job_defaults_without_optional_execution_policy_keys() {
    let cx = context();
    let pod = PodPlan::new(
        "backup",
        ContainerPlan::new("backup", "demo:1", vec!["backup".into()]),
        PodRuntimePolicy::restricted("sample-backup", json!({})),
    );
    let cron = CronJobPlan::new("sample-backup-defaults", "0 2 * * *", pod);
    assert_eq!(cron.suspend(), None);
    assert_eq!(cron.starting_deadline_seconds(), None);
    assert_eq!(cron.active_deadline_seconds(), None);
    assert_eq!(cron.backoff_limit(), None);
    assert_eq!(cron.ttl_seconds_after_finished(), None);

    let mut plan = WorkloadPlan::new(&cx);
    plan.add_cron_job(cron);
    let objects = plan.render().expect("render cron job with defaults");
    assert_eq!(objects.len(), 1);

    let spec = &objects[0]["spec"];
    assert_eq!(spec["schedule"], "0 2 * * *");
    assert_eq!(spec["concurrencyPolicy"], "Forbid");
    assert_eq!(spec["successfulJobsHistoryLimit"], 3);
    assert_eq!(spec["failedJobsHistoryLimit"], 1);
    assert!(spec.get("suspend").is_none());
    assert!(spec.get("startingDeadlineSeconds").is_none());

    let job_spec = &spec["jobTemplate"]["spec"];
    assert!(job_spec.get("activeDeadlineSeconds").is_none());
    assert!(job_spec.get("backoffLimit").is_none());
    assert!(job_spec.get("ttlSecondsAfterFinished").is_none());
    assert_eq!(job_spec["template"]["spec"]["restartPolicy"], "OnFailure");
}

#[test]
fn typed_plan_renders_cron_job_with_all_execution_policy_knobs() {
    let cx = context();
    let pod = PodPlan::new(
        "backup",
        ContainerPlan::new("backup", "demo:1", vec!["backup".into()]),
        PodRuntimePolicy::restricted("sample-backup", json!({})),
    );
    let cron = CronJobPlan::new("sample-backup-configured", "0 4 * * *", pod)
        .with_suspend(true)
        .with_starting_deadline_seconds(120)
        .with_active_deadline_seconds(3600)
        .with_backoff_limit(4)
        .with_ttl_seconds_after_finished(86400);

    assert_eq!(cron.suspend(), Some(true));
    assert_eq!(cron.starting_deadline_seconds(), Some(120));
    assert_eq!(cron.active_deadline_seconds(), Some(3600));
    assert_eq!(cron.backoff_limit(), Some(4));
    assert_eq!(cron.ttl_seconds_after_finished(), Some(86400));

    let mut plan = WorkloadPlan::new(&cx);
    plan.add_cron_job(cron);
    let objects = plan.render().expect("render configured cron job");
    assert_eq!(objects.len(), 1);

    let spec = &objects[0]["spec"];
    assert_eq!(spec["schedule"], "0 4 * * *");
    assert_eq!(spec["concurrencyPolicy"], "Forbid");
    assert_eq!(spec["suspend"], json!(true));
    assert_eq!(spec["startingDeadlineSeconds"], json!(120));

    let job_spec = &spec["jobTemplate"]["spec"];
    assert_eq!(job_spec["activeDeadlineSeconds"], json!(3600));
    assert_eq!(job_spec["backoffLimit"], json!(4));
    assert_eq!(job_spec["ttlSecondsAfterFinished"], json!(86400));
    assert_eq!(job_spec["template"]["spec"]["restartPolicy"], "OnFailure");
}

#[test]
fn typed_plan_rejects_negative_cron_job_execution_policy_knobs() {
    let cx = context();
    let make_pod = || {
        PodPlan::new(
            "backup",
            ContainerPlan::new("backup", "demo:1", vec!["backup".into()]),
            PodRuntimePolicy::restricted("sample-backup", json!({})),
        )
    };

    // Negative startingDeadlineSeconds
    let mut plan1 = WorkloadPlan::new(&cx);
    plan1.add_cron_job(
        CronJobPlan::new("sample-cron", "0 1 * * *", make_pod()).with_starting_deadline_seconds(-1),
    );
    assert_eq!(
        plan1.render(),
        Err(WorkloadPlanError::NegativeValue(
            "sample-cron".into(),
            "startingDeadlineSeconds",
        ))
    );

    // Negative activeDeadlineSeconds
    let mut plan2 = WorkloadPlan::new(&cx);
    plan2.add_cron_job(
        CronJobPlan::new("sample-cron", "0 1 * * *", make_pod()).with_active_deadline_seconds(-1),
    );
    assert_eq!(
        plan2.render(),
        Err(WorkloadPlanError::NegativeValue(
            "sample-cron".into(),
            "activeDeadlineSeconds",
        ))
    );

    // Negative backoffLimit
    let mut plan3 = WorkloadPlan::new(&cx);
    plan3.add_cron_job(
        CronJobPlan::new("sample-cron", "0 1 * * *", make_pod()).with_backoff_limit(-1),
    );
    assert_eq!(
        plan3.render(),
        Err(WorkloadPlanError::NegativeValue(
            "sample-cron".into(),
            "backoffLimit",
        ))
    );

    // Negative ttlSecondsAfterFinished
    let mut plan4 = WorkloadPlan::new(&cx);
    plan4.add_cron_job(
        CronJobPlan::new("sample-cron", "0 1 * * *", make_pod())
            .with_ttl_seconds_after_finished(-1),
    );
    assert_eq!(
        plan4.render(),
        Err(WorkloadPlanError::NegativeValue(
            "sample-cron".into(),
            "ttlSecondsAfterFinished",
        ))
    );
}
