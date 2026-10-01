use super::*;

fn plan(storage: StatefulStorageAttachment) -> StatefulInstancePlan<'static> {
    let cx = Box::leak(Box::new(RenderCtx::new(
        "lumen", "test", "v1", "Lumen", "lumen", "lumen",
    )));
    let pod = ServicePodTemplate {
        cx,
        component: "serving",
        image: "lumen",
        image_pull_policy: "IfNotPresent",
        command: vec!["lumen".into()],
        args: vec![],
        ports: vec![],
        env: vec![],
        env_from: vec![],
        resources: json!({}),
        readiness_probe: None,
        liveness_probe: None,
        startup_probe: None,
        lifecycle: None,
        container_security_context: None,
        pod_security_context: None,
        service_account_name: None,
        termination_grace_period_seconds: None,
        volumes: vec![],
        volume_mounts: vec![],
        pod_annotations: None,
        topology_spread_constraints: vec![],
    };
    StatefulInstancePlan::new(cx, "lumen", 1, pod, storage)
}

fn template_claim() -> VolumeClaimTemplate {
    VolumeClaimTemplate::new(
        "data",
        json!({"spec": {"resources": {"requests": {"storage": "20Gi"}}}}),
        "/data",
    )
}

fn existing_claim() -> ExistingClaim {
    ExistingClaim::new(
        "data",
        "data",
        json!({"spec": {"resources": {"requests": {"storage": "20Gi"}}}}),
        "/data",
    )
}

#[test]
fn service_links_are_opt_in_and_explicit() {
    let default = plan(StatefulStorageAttachment::VolumeClaimTemplate(
        template_claim(),
    ));
    assert!(default.enable_service_links.is_none());
    assert!(
        stateful_instance(default).unwrap().workload["spec"]["template"]["spec"]
            .get("enableServiceLinks")
            .is_none()
    );

    let mut disabled = plan(StatefulStorageAttachment::VolumeClaimTemplate(
        template_claim(),
    ));
    disabled.enable_service_links = Some(false);
    assert_eq!(
        stateful_instance(disabled).unwrap().workload["spec"]["template"]["spec"]
            ["enableServiceLinks"],
        false
    );

    let mut enabled = plan(StatefulStorageAttachment::VolumeClaimTemplate(
        template_claim(),
    ));
    enabled.enable_service_links = Some(true);
    assert_eq!(
        stateful_instance(enabled).unwrap().workload["spec"]["template"]["spec"]
            ["enableServiceLinks"],
        true
    );
}

#[test]
fn template_attachment_is_rendered_by_the_statefulset() {
    let rendered = stateful_instance(plan(StatefulStorageAttachment::VolumeClaimTemplate(
        template_claim(),
    )))
    .unwrap();
    assert!(rendered.storage.is_none());
    assert_eq!(
        rendered.workload["spec"]["volumeClaimTemplates"][0]["metadata"]["name"],
        "data"
    );
    assert_eq!(
        rendered.workload["spec"]["template"]["spec"]["containers"][0]["volumeMounts"][0]
            ["mountPath"],
        "/data"
    );
}

#[test]
fn existing_claim_is_an_independent_pvc_and_pod_volume() {
    let rendered = stateful_instance(plan(StatefulStorageAttachment::ExistingClaim(
        existing_claim(),
    )))
    .unwrap();
    assert!(rendered.workload["spec"]["volumeClaimTemplates"].is_null());
    assert_eq!(
        rendered.workload["spec"]["template"]["spec"]["volumes"][0]["persistentVolumeClaim"]
            ["claimName"],
        "data"
    );
    assert_eq!(rendered.storage.unwrap()["metadata"]["name"], "data");
}

#[test]
fn invalid_replica_and_selector_are_rejected() {
    let mut zero = plan(StatefulStorageAttachment::VolumeClaimTemplate(
        template_claim(),
    ));
    zero.replicas = 0;
    assert_eq!(
        stateful_instance(zero),
        Err(StatefulInstanceError::ZeroReplicas)
    );
    let mut override_selector = plan(StatefulStorageAttachment::VolumeClaimTemplate(
        template_claim(),
    ));
    override_selector
        .selector
        .insert("app.kubernetes.io/name".into(), "other".into());
    assert_eq!(
        stateful_instance(override_selector),
        Err(StatefulInstanceError::SelectorCoreIdentityOverride)
    );
}
