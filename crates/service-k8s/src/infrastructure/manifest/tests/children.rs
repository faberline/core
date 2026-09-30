use super::*;

#[test]
fn helper_shapes() {
    let cx = cx();
    assert_eq!(service_account(&cx, "server")["kind"], "ServiceAccount");
    let h = headless_service_with_ports(
        &cx,
        "s-h",
        "server",
        vec![
            json!({ "name": "http", "port": 7000, "targetPort": "http", "protocol": "TCP" }),
            json!({ "name": "grpc", "port": 7443, "targetPort": "grpc", "protocol": "TCP" }),
        ],
    );
    assert_eq!(h["spec"]["clusterIP"], "None");
    assert_eq!(h["spec"]["publishNotReadyAddresses"], true);
    assert_eq!(h["spec"]["ports"].as_array().unwrap().len(), 2);
    assert_eq!(
        client_service_with_ports(
            &cx,
            "s",
            "server",
            vec![json!({ "name": "http", "port": 7000, "targetPort": "http", "protocol": "TCP" })],
        )["spec"]["type"],
        "ClusterIP"
    );
    assert_eq!(pdb(&cx, "s", "server", 1)["spec"]["maxUnavailable"], 1);
    // labels carry the per-service manager.
    assert_eq!(
        cx.labels("server")["app.kubernetes.io/managed-by"],
        "svc-operator"
    );
    assert_eq!(restricted_pod_security_context()["runAsNonRoot"], true);
    assert_eq!(
        restricted_pod_security_context()["seccompProfile"]["type"],
        "RuntimeDefault"
    );
    assert_eq!(
        restricted_container_security_context()["readOnlyRootFilesystem"],
        true
    );
    assert_eq!(
        restricted_container_security_context()["capabilities"]["drop"][0],
        "ALL"
    );

    let secret = TokenRegistryProjection {
        volume_name: "registry",
        mount_path: "/var/run/secrets/svc",
        source: TokenRegistrySource::Secret {
            name: "svc-registry",
            key: "token-registry.json",
        },
    };
    assert_eq!(token_registry_mount(&secret)["readOnly"], true);
    assert_eq!(
        token_registry_volume(&secret)["secret"]["secretName"],
        "svc-registry"
    );

    let csi = TokenRegistryProjection {
        volume_name: "registry",
        mount_path: "/var/run/secrets/svc",
        source: TokenRegistrySource::Csi {
            provider_class: "svc-registry",
            driver: None,
        },
    };
    assert_eq!(
        token_registry_volume(&csi)["csi"]["volumeAttributes"]["secretProviderClass"],
        "svc-registry"
    );
}

#[test]
fn token_registry_volume_csi_driver_defaults_to_vanilla_and_can_be_overridden() {
    let default_csi = TokenRegistryProjection {
        volume_name: "registry",
        mount_path: "/var/run/secrets/svc",
        source: TokenRegistrySource::Csi {
            provider_class: "svc-registry",
            driver: None,
        },
    };
    assert_eq!(
        token_registry_volume(&default_csi)["csi"]["driver"],
        "secrets-store.csi.k8s.io"
    );

    let gke_csi = TokenRegistryProjection {
        volume_name: "registry",
        mount_path: "/var/run/secrets/svc",
        source: TokenRegistrySource::Csi {
            provider_class: "svc-registry",
            driver: Some("secrets-store-gke.csi.k8s.io"),
        },
    };
    assert_eq!(
        token_registry_volume(&gke_csi)["csi"]["driver"],
        "secrets-store-gke.csi.k8s.io"
    );
}

#[test]
fn cron_job_wires_runner_without_domain_bytes() {
    let cx = cx();
    let cj = cron_job(CronJob {
        cx: &cx,
        name: "s-backup",
        component: "backup",
        schedule: "*/5 * * * *",
        image: "svc:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["svc".into()],
        args: vec!["backup".into(), "run".into()],
        env: vec![json!({ "name": "DESTINATION", "value": "s3://bucket/prefix" })],
        env_from: vec![json!({ "secretRef": { "name": "s-backup" } })],
        volumes: vec![json!({ "name": "token", "projected": {} })],
        volume_mounts: vec![json!({ "name": "token", "mountPath": "/var/run/secrets" })],
        service_account_name: Some("s-backup"),
        cpu: "100m",
        memory: "128Mi",
        successful_jobs_history_limit: 1,
        failed_jobs_history_limit: 3,
    });
    assert_eq!(cj["kind"], "CronJob");
    assert_eq!(cj["spec"]["concurrencyPolicy"], "Forbid");
    assert_eq!(cj["spec"]["schedule"], "*/5 * * * *");
    let pod = &cj["spec"]["jobTemplate"]["spec"]["template"]["spec"];
    assert_eq!(pod["serviceAccountName"], "s-backup");
    assert_eq!(pod["restartPolicy"], "OnFailure");
    assert_eq!(
        pod["containers"][0]["env"][0]["value"],
        "s3://bucket/prefix"
    );
    assert_eq!(
        pod["containers"][0]["envFrom"][0]["secretRef"]["name"],
        "s-backup"
    );
    assert_eq!(pod["volumes"][0]["name"], "token");
}

#[test]
fn horizontal_pod_autoscaler_renders_expected_shape() {
    let cx = cx();
    let hpa = horizontal_pod_autoscaler(HorizontalPodAutoscaler {
        cx: &cx,
        name: "s",
        component: "server",
        target_api_version: "apps/v1",
        target_kind: "StatefulSet",
        target_name: "s",
        min_replicas: 2,
        max_replicas: 8,
        metrics: vec![json!({
            "type": "Resource",
            "resource": {
                "name": "cpu",
                "target": { "type": "Utilization", "averageUtilization": 70 }
            }
        })],
        behavior: Some(json!({
            "scaleUp": {
                "stabilizationWindowSeconds": 30,
                "policies": [{ "type": "Percent", "value": 100, "periodSeconds": 30 }]
            }
        })),
    });
    assert_eq!(hpa["kind"], "HorizontalPodAutoscaler");
    assert_eq!(hpa["spec"]["scaleTargetRef"]["kind"], "StatefulSet");
    assert_eq!(hpa["spec"]["minReplicas"], 2);
    assert_eq!(
        hpa["spec"]["metrics"][0]["resource"]["target"]["averageUtilization"],
        70
    );
    assert_eq!(
        hpa["spec"]["behavior"]["scaleUp"]["policies"][0]["periodSeconds"],
        30
    );
}

#[test]
fn render_ctx_getters_return_the_constructor_arguments_in_order() {
    let cx = RenderCtx::new("app", "manager", "g.dev/v1", "Kind", "name", "ns");
    assert_eq!(
        [
            cx.app(),
            cx.manager(),
            cx.api_version(),
            cx.kind(),
            cx.name(),
            cx.ns()
        ],
        ["app", "manager", "g.dev/v1", "Kind", "name", "ns"]
    );
    assert!(cx.owner().is_none());
    assert!(cx.meta("child", "c").get("ownerReferences").is_none());

    let owner = owner_ref("g.dev/v1", "Kind", "name", "uid-1");
    let cx = cx.with_owner(owner.clone());
    assert_eq!(cx.owner(), Some(&owner));
    assert_eq!(cx.meta("child", "c")["ownerReferences"], json!([owner]));
}
