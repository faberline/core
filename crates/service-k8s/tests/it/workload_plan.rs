use std::collections::BTreeMap;

use serde_json::json;
use service_k8s::render::{
    ClusterRoleBindingPlan, ContainerPlan, CronJobPlan, DeploymentPlan, FqdnMatchPlan,
    FqdnNetworkPolicyPlan, NetworkPeerPlan, NetworkPolicyPlan, NetworkPortPlan, NetworkRulePlan,
    PodPlan, PodRuntimePolicy, ProjectedServiceAccountToken, RbacRulePlan, RenderCtx,
    RoleBindingPlan, RolePlan, ServiceAccountIdentityPlan, ServiceAccountPlan,
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
fn typed_plan_renders_identity_separated_backup_and_restore_service_accounts() {
    let cx = context();

    let backup_sa = ServiceAccountIdentityPlan::new("sample-backup-sa", "backup")
        .with_workload_identity("backup-agent@demo-project.iam.gserviceaccount.com");
    assert_eq!(backup_sa.name(), "sample-backup-sa");
    assert_eq!(backup_sa.component(), "backup");
    assert_eq!(
        backup_sa.workload_identity(),
        Some("backup-agent@demo-project.iam.gserviceaccount.com")
    );
    assert_eq!(backup_sa.automount_service_account_token(), false);

    let restore_sa = ServiceAccountIdentityPlan::new("sample-restore-sa", "restore")
        .with_workload_identity("restore-agent@demo-project.iam.gserviceaccount.com");
    assert_eq!(restore_sa.name(), "sample-restore-sa");
    assert_eq!(restore_sa.component(), "restore");
    assert_eq!(
        restore_sa.workload_identity(),
        Some("restore-agent@demo-project.iam.gserviceaccount.com")
    );
    assert_eq!(restore_sa.automount_service_account_token(), false);

    let backup_role = RolePlan::new("sample-backup-role", "backup").with_rule(RbacRulePlan::new(
        vec!["".into()],
        vec!["pods".into()],
        vec!["get".into(), "list".into()],
    ));
    let restore_role =
        RolePlan::new("sample-restore-role", "restore").with_rule(RbacRulePlan::new(
            vec!["apps".into()],
            vec!["statefulsets".into()],
            vec!["get".into(), "patch".into()],
        ));

    // Assert the two Roles share no rule
    let backup_rules = backup_role.rules();
    let restore_rules = restore_role.rules();
    for b_rule in backup_rules {
        for r_rule in restore_rules {
            assert_ne!(b_rule.api_groups(), r_rule.api_groups());
            assert_ne!(b_rule.resources(), r_rule.resources());
        }
    }

    let backup_binding =
        RoleBindingPlan::new("sample-backup-binding", "backup", "sample-backup-role")
            .with_service_account(ServiceAccountSubjectPlan::new(
                "observability",
                "sample-backup-sa",
            ));

    let restore_binding =
        RoleBindingPlan::new("sample-restore-binding", "restore", "sample-restore-role")
            .with_service_account(ServiceAccountSubjectPlan::new(
                "observability",
                "sample-restore-sa",
            ));

    let mut plan = WorkloadPlan::new(&cx);
    plan.add_service_account_identity(backup_sa);
    plan.add_role(backup_role);
    plan.add_role_binding(backup_binding);
    plan.add_service_account_identity(restore_sa);
    plan.add_role(restore_role);
    plan.add_role_binding(restore_binding);

    let objects = plan.render().expect("render typed workload plan");
    assert_eq!(objects.len(), 6);

    // Backup ServiceAccount
    assert_eq!(objects[0]["kind"], "ServiceAccount");
    assert_eq!(objects[0]["metadata"]["name"], "sample-backup-sa");
    assert_eq!(
        objects[0]["metadata"]["annotations"]["iam.gke.io/gcp-service-account"],
        "backup-agent@demo-project.iam.gserviceaccount.com"
    );
    assert_eq!(objects[0]["automountServiceAccountToken"], false);

    // Backup Role & Binding
    assert_eq!(objects[1]["kind"], "Role");
    assert_eq!(objects[1]["metadata"]["name"], "sample-backup-role");
    assert_eq!(objects[2]["kind"], "RoleBinding");
    assert_eq!(objects[2]["roleRef"]["name"], "sample-backup-role");
    assert_eq!(
        objects[2]["subjects"],
        json!([{
            "kind": "ServiceAccount",
            "name": "sample-backup-sa",
            "namespace": "observability"
        }])
    );

    // Restore ServiceAccount
    assert_eq!(objects[3]["kind"], "ServiceAccount");
    assert_eq!(objects[3]["metadata"]["name"], "sample-restore-sa");
    assert_eq!(
        objects[3]["metadata"]["annotations"]["iam.gke.io/gcp-service-account"],
        "restore-agent@demo-project.iam.gserviceaccount.com"
    );
    assert_eq!(objects[3]["automountServiceAccountToken"], false);

    // Restore Role & Binding
    assert_eq!(objects[4]["kind"], "Role");
    assert_eq!(objects[4]["metadata"]["name"], "sample-restore-role");
    assert_eq!(objects[5]["kind"], "RoleBinding");
    assert_eq!(objects[5]["roleRef"]["name"], "sample-restore-role");
    assert_eq!(
        objects[5]["subjects"],
        json!([{
            "kind": "ServiceAccount",
            "name": "sample-restore-sa",
            "namespace": "observability"
        }])
    );
}

#[test]
fn typed_plan_renders_pod_with_projected_service_account_token_and_no_secret_volume() {
    let cx = context();
    let caller_audience = "https://admin.service.observability.svc.cluster.local";
    let token = ProjectedServiceAccountToken::new(
        "admin-api-token",
        "/var/run/secrets/service/tokens",
        caller_audience,
    );

    let mut container = ContainerPlan::new("backup-agent", "backup:1", vec!["run-backup".into()]);
    container.volume_mounts.push(token.mount());

    let mut runtime = PodRuntimePolicy::restricted("sample-backup-sa", json!({}));
    runtime.volumes.push(token.volume());

    let pod = PodPlan::new("backup", container, runtime);

    let mut plan = WorkloadPlan::new(&cx);
    plan.add_deployment(DeploymentPlan::new("sample-backup", 1, pod));

    let objects = plan.render().expect("render typed workload plan");
    assert_eq!(objects.len(), 1);
    let deployment = &objects[0];
    assert_eq!(deployment["kind"], "Deployment");

    let pod_spec = &deployment["spec"]["template"]["spec"];

    // Verify volumes: projected volume present, no Secret volume
    let volumes = pod_spec["volumes"].as_array().expect("volumes array");
    assert_eq!(volumes.len(), 1);
    let vol = &volumes[0];
    assert_eq!(vol["name"], "admin-api-token");
    assert!(vol.get("projected").is_some(), "volume must be projected");
    assert!(
        vol.get("secret").is_none(),
        "no Secret volume must be rendered"
    );

    let sources = vol["projected"]["sources"]
        .as_array()
        .expect("sources array");
    assert_eq!(sources.len(), 1);
    let sa_token = &sources[0]["serviceAccountToken"];
    assert_eq!(sa_token["audience"], caller_audience);
    let expiration_seconds = sa_token["expirationSeconds"].as_u64().expect("expiration");
    assert!(
        expiration_seconds >= 600,
        "expirationSeconds must be >= 600, got {expiration_seconds}"
    );

    // Verify container volume mounts
    let mounts = pod_spec["containers"][0]["volumeMounts"]
        .as_array()
        .expect("volumeMounts");
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0]["name"], "admin-api-token");
    assert_eq!(mounts[0]["mountPath"], "/var/run/secrets/service/tokens");
    assert_eq!(mounts[0]["readOnly"], true);
}

#[test]
fn service_account_identity_without_annotations_omits_annotations_key_and_equals_service_account_plan(
) {
    let cx = context();

    let mut identity_plan = WorkloadPlan::new(&cx);
    identity_plan.add_service_account_identity(ServiceAccountIdentityPlan::new(
        "sample-identity-sa",
        "agent",
    ));
    let identity_objects = identity_plan.render().expect("render identity plan");
    assert_eq!(identity_objects.len(), 1);

    let identity_rendered = &identity_objects[0];
    assert_eq!(identity_rendered["kind"], "ServiceAccount");
    assert_eq!(identity_rendered["metadata"]["name"], "sample-identity-sa");
    assert!(
        identity_rendered["metadata"].get("annotations").is_none(),
        "metadata.annotations must be omitted when no annotations are set"
    );
    assert_eq!(identity_rendered["automountServiceAccountToken"], false);

    // Verify it equals the existing ServiceAccountPlan output
    let mut baseline_plan = WorkloadPlan::new(&cx);
    baseline_plan.add_service_account(ServiceAccountPlan::new("sample-identity-sa", "agent"));
    let baseline_objects = baseline_plan.render().expect("render baseline plan");
    assert_eq!(baseline_objects.len(), 1);

    assert_eq!(identity_rendered, &baseline_objects[0]);
}

#[test]
fn service_account_identity_rejects_empty_or_invalid_workload_identity_shape() {
    let cx = context();

    // Rejection of empty value via WorkloadPlan::render
    let mut plan1 = WorkloadPlan::new(&cx);
    plan1.add_service_account_identity(
        ServiceAccountIdentityPlan::new("sample-sa", "backup").with_workload_identity(""),
    );
    assert_eq!(
        plan1.render(),
        Err(WorkloadPlanError::InvalidWorkloadIdentity(
            "sample-sa".into(),
            "".into(),
        ))
    );

    // Rejection of missing @ / invalid shape via WorkloadPlan::render
    let mut plan2 = WorkloadPlan::new(&cx);
    plan2.add_service_account_identity(
        ServiceAccountIdentityPlan::new("sample-sa", "backup")
            .with_workload_identity("not-an-email"),
    );
    assert_eq!(
        plan2.render(),
        Err(WorkloadPlanError::InvalidWorkloadIdentity(
            "sample-sa".into(),
            "not-an-email".into(),
        ))
    );

    // Rejection of invalid domain suffix via WorkloadPlan::render
    let mut plan3 = WorkloadPlan::new(&cx);
    plan3.add_service_account_identity(
        ServiceAccountIdentityPlan::new("sample-sa", "backup")
            .with_workload_identity("user@otherdomain.com"),
    );
    assert_eq!(
        plan3.render(),
        Err(WorkloadPlanError::InvalidWorkloadIdentity(
            "sample-sa".into(),
            "user@otherdomain.com".into(),
        ))
    );

    // Rejection of empty name via WorkloadPlan::render
    let mut plan4 = WorkloadPlan::new(&cx);
    plan4.add_service_account_identity(
        ServiceAccountIdentityPlan::new("sample-sa", "backup")
            .with_workload_identity("@demo-project.iam.gserviceaccount.com"),
    );
    assert_eq!(
        plan4.render(),
        Err(WorkloadPlanError::InvalidWorkloadIdentity(
            "sample-sa".into(),
            "@demo-project.iam.gserviceaccount.com".into(),
        ))
    );

    // Rejection of empty project via WorkloadPlan::render
    let mut plan5 = WorkloadPlan::new(&cx);
    plan5.add_service_account_identity(
        ServiceAccountIdentityPlan::new("sample-sa", "backup")
            .with_workload_identity("backup@.iam.gserviceaccount.com"),
    );
    assert_eq!(
        plan5.render(),
        Err(WorkloadPlanError::InvalidWorkloadIdentity(
            "sample-sa".into(),
            "backup@.iam.gserviceaccount.com".into(),
        ))
    );

    // Also covers builder return Result via try_with_workload_identity
    let builder_err = ServiceAccountIdentityPlan::new("sample-sa", "backup")
        .try_with_workload_identity("invalid-shape");
    assert_eq!(
        builder_err,
        Err(WorkloadPlanError::InvalidWorkloadIdentity(
            "sample-sa".into(),
            "invalid-shape".into(),
        ))
    );
}
