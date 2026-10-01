use super::*;

#[test]
fn service_statefulset_keeps_exact_downward_api_env_contract() {
    let cx = cx();
    let ss = service_statefulset(ServiceStatefulSet {
        cx: &cx,
        name: "s",
        component: "server",
        image: "img:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["serve".into()],
        args: vec![],
        ports: vec![json!({ "name": "http", "containerPort": 7000, "protocol": "TCP" })],
        headless_service: "s-headless",
        shard_count: 2,
        replicas_per_shard: 3,
        voter_count: 3,
        headless_env_key: "SVC_HEADLESS_SERVICE",
        service_account_name: Some("s"),
        env: vec![json!({ "name": "EXTRA", "value": "x" })],
        env_from: vec![],
        resources: guaranteed_resources("1", "1Gi"),
        pod_annotations: None,
        pod_security_context: None,
        container_security_context: None,
        termination_grace_period_seconds: None,
        readiness_probe: None,
        liveness_probe: None,
        startup_probe: None,
        lifecycle: None,
        volumes: vec![],
        volume_mounts: vec![],
        affinity: None,
        node_selector: None,
        tolerations: vec![],
        topology_spread_constraints: vec![],
        revision_history_limit: None,
        update_strategy: None,
        volume_claim: None,
    });
    assert_eq!(ss["spec"]["replicas"], 6); // shard_count * replicas_per_shard
    assert_eq!(ss["spec"]["serviceName"], "s-headless");
    assert_eq!(ss["spec"]["podManagementPolicy"], "Parallel");
    let env = ss["spec"]["template"]["spec"]["containers"][0]["env"]
        .as_array()
        .unwrap();
    let keys: Vec<&str> = env.iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert_eq!(
        &keys[..6],
        &[
            ENV_POD_NAME,
            ENV_POD_NAMESPACE,
            ENV_SHARD_COUNT,
            ENV_REPLICAS_PER_SHARD,
            ENV_VOTER_COUNT,
            "SVC_HEADLESS_SERVICE",
        ]
    );
    for k in [
        ENV_POD_NAME,
        ENV_POD_NAMESPACE,
        ENV_SHARD_COUNT,
        ENV_REPLICAS_PER_SHARD,
        ENV_VOTER_COUNT,
        "SVC_HEADLESS_SERVICE",
        "EXTRA",
    ] {
        assert!(keys.contains(&k), "missing env {k}");
    }
    // the field-ref quartet members use the downward API, not a literal value.
    let pod_name = env.iter().find(|e| e["name"] == ENV_POD_NAME).unwrap();
    assert_eq!(
        pod_name["valueFrom"]["fieldRef"]["fieldPath"],
        "metadata.name"
    );
}

#[test]
fn service_statefulset_can_render_lumen_style_production_template() {
    let cx = cx();
    let spread = |key: &str| {
        json!({
            "maxSkew": 1,
            "topologyKey": key,
            "whenUnsatisfiable": "ScheduleAnyway",
            "labelSelector": { "matchLabels": cx.selector("server") },
        })
    };
    let ss = service_statefulset(ServiceStatefulSet {
        cx: &cx,
        name: "s",
        component: "server",
        image: "img:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["lumen".into(), "serve".into()],
        args: vec![],
        ports: vec![json!({ "name": "http", "containerPort": 7373, "protocol": "TCP" })],
        headless_service: "s-headless",
        shard_count: 1,
        replicas_per_shard: 1,
        voter_count: 1,
        headless_env_key: "LUMEN_HEADLESS_SERVICE",
        service_account_name: Some("s"),
        env: vec![],
        env_from: vec![json!({ "configMapRef": { "name": "s-config" } })],
        resources: requested_resources("2", "4Gi"),
        pod_annotations: Some(json!({
            "prometheus.io/scrape": "true",
            "prometheus.io/port": "7373",
            "prometheus.io/path": "/metrics",
        })),
        pod_security_context: Some(json!({
            "runAsNonRoot": true,
            "runAsUser": 65532,
            "runAsGroup": 65532,
            "fsGroup": 65532,
            "seccompProfile": { "type": "RuntimeDefault" },
        })),
        container_security_context: Some(json!({
            "runAsNonRoot": true,
            "runAsUser": 65532,
            "runAsGroup": 65532,
            "allowPrivilegeEscalation": false,
            "readOnlyRootFilesystem": true,
            "capabilities": { "drop": ["ALL"] },
        })),
        termination_grace_period_seconds: Some(30),
        readiness_probe: Some(json!({
            "httpGet": { "path": "/readyz", "port": "http" },
            "initialDelaySeconds": 5, "periodSeconds": 10, "timeoutSeconds": 3, "failureThreshold": 60,
        })),
        liveness_probe: Some(json!({
            "httpGet": { "path": "/healthz", "port": "http" },
            "initialDelaySeconds": 15, "periodSeconds": 30, "timeoutSeconds": 5, "failureThreshold": 3,
        })),
        startup_probe: Some(json!({
            "httpGet": { "path": "/healthz", "port": "http" },
            "periodSeconds": 5, "timeoutSeconds": 3, "failureThreshold": 120,
        })),
        lifecycle: None,
        volumes: vec![json!({ "name": "tmp", "emptyDir": {} })],
        volume_mounts: vec![json!({ "name": "tmp", "mountPath": "/tmp" })],
        affinity: Some(dedicated_node_affinity(cx.selector("server"))),
        node_selector: None,
        tolerations: vec![],
        topology_spread_constraints: vec![
            spread("topology.kubernetes.io/zone"),
            spread("kubernetes.io/hostname"),
        ],
        revision_history_limit: Some(5),
        update_strategy: Some(json!({ "type": "RollingUpdate" })),
        volume_claim: Some(WorkloadVolumeClaim {
            name: "raft".into(),
            template: json!({
                "spec": {
                    "accessModes": ["ReadWriteOnce"],
                    "resources": { "requests": { "storage": "100Gi" } },
                },
            }),
            mount_path: "/var/lib/lumen",
            read_only: false,
        }),
    });
    let pod = &ss["spec"]["template"]["spec"];
    let container = &pod["containers"][0];
    assert_eq!(
        ss["spec"]["template"]["metadata"]["annotations"]["prometheus.io/path"],
        "/metrics"
    );
    assert_eq!(pod["serviceAccountName"], "s");
    assert_eq!(pod["terminationGracePeriodSeconds"], 30);
    assert_eq!(
        pod["securityContext"]["seccompProfile"]["type"],
        "RuntimeDefault"
    );
    assert_eq!(
        pod["topologySpreadConstraints"].as_array().unwrap().len(),
        2
    );
    assert_eq!(container["resources"]["requests"]["memory"], "4Gi");
    assert!(container["resources"].get("limits").is_none());
    assert_eq!(
        pod["affinity"]["podAntiAffinity"]["requiredDuringSchedulingIgnoredDuringExecution"][0]
            ["topologyKey"],
        "kubernetes.io/hostname"
    );
    assert_eq!(container["envFrom"][0]["configMapRef"]["name"], "s-config");
    assert_eq!(container["readinessProbe"]["httpGet"]["path"], "/readyz");
    assert_eq!(container["securityContext"]["readOnlyRootFilesystem"], true);
    let env = container["env"].as_array().unwrap();
    assert!(!env
        .iter()
        .any(|entry| entry["name"] == "LUMEN_TOKEN_REGISTRY_FILE"));
    let volumes = pod["volumes"].as_array().unwrap();
    assert_eq!(volumes.len(), 1);
    assert_eq!(volumes[0]["name"], "tmp");
    assert_eq!(volumes[0]["emptyDir"], json!({}));
    assert!(!volumes
        .iter()
        .any(|volume| volume["name"] == "token-registry"));
    assert!(!volumes
        .iter()
        .any(|volume| volume["secret"]["secretName"] == "lumen-token-registry"));
    let mounts = container["volumeMounts"].as_array().unwrap();
    assert_eq!(mounts.len(), 2);
    assert_eq!(mounts[0]["name"], "tmp");
    assert_eq!(mounts[0]["mountPath"], "/tmp");
    assert_eq!(mounts[1]["name"], "raft");
    assert_eq!(mounts[1]["mountPath"], "/var/lib/lumen");
    assert_eq!(mounts[1]["readOnly"], false);
    assert!(!mounts.iter().any(|mount| mount["name"] == "token-registry"));
    assert!(!mounts
        .iter()
        .any(|mount| mount["mountPath"] == "/var/run/secrets/lumen"));
    assert_eq!(
        ss["spec"]["volumeClaimTemplates"][0]["metadata"]["name"],
        "raft"
    );
    assert_eq!(
        ss["spec"]["volumeClaimTemplates"][0]["metadata"]["labels"]["app.kubernetes.io/name"],
        "svc"
    );
}

#[test]
fn sharded_statefulset_legacy_wrapper_keeps_default_claim_mount() {
    let cx = cx();
    let ss = sharded_statefulset(ShardedStatefulSet {
        cx: &cx,
        name: "s",
        component: "server",
        image: "img:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["serve".into()],
        ports: vec![("http", 7000)],
        headless_service: "s-headless",
        shard_count: 1,
        replicas_per_shard: 1,
        voter_count: 1,
        headless_env_key: "SVC_HEADLESS_SERVICE",
        cpu: "",
        memory: "",
        extra_env: vec![],
        volume_claim: Some(json!({ "metadata": { "name": "data" }, "spec": {} })),
    });
    assert!(ss["spec"]["volumeClaimTemplates"].is_array());
    let pod = &ss["spec"]["template"]["spec"];
    let resources = &pod["containers"][0]["resources"];
    assert_eq!(resources["requests"]["cpu"], "1");
    assert_eq!(resources["requests"]["memory"], "4Gi");
    assert!(resources.get("limits").is_none());
    assert_eq!(
        pod["affinity"]["podAntiAffinity"]["requiredDuringSchedulingIgnoredDuringExecution"][0]
            ["topologyKey"],
        "kubernetes.io/hostname"
    );
    assert_eq!(
        ss["spec"]["template"]["spec"]["containers"][0]["volumeMounts"][0]["mountPath"],
        "/data"
    );
}
