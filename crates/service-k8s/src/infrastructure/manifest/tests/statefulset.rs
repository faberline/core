use super::*;

use crate::infrastructure::manifest::stateful_instance::StatefulInstanceError;

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
    })
    .expect("valid statefulset");
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
    }).expect("valid statefulset");
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
    })
    .expect("valid statefulset");
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

#[test]
fn service_statefulset_zero_shard_count_returns_error() {
    let cx = cx();
    let p = ServiceStatefulSet {
        cx: &cx,
        name: "s",
        component: "server",
        image: "img:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["serve".into()],
        args: vec![],
        ports: vec![json!({ "name": "http", "containerPort": 7000, "protocol": "TCP" })],
        headless_service: "s-headless",
        shard_count: 0,
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
    };
    assert_eq!(
        service_statefulset(p),
        Err(StatefulInstanceError::ZeroReplicas)
    );
}

#[test]
fn service_statefulset_zero_replicas_per_shard_returns_error() {
    let cx = cx();
    let p = ServiceStatefulSet {
        cx: &cx,
        name: "s",
        component: "server",
        image: "img:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["serve".into()],
        args: vec![],
        ports: vec![json!({ "name": "http", "containerPort": 7000, "protocol": "TCP" })],
        headless_service: "s-headless",
        shard_count: 3,
        replicas_per_shard: 0,
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
    };
    assert_eq!(
        service_statefulset(p),
        Err(StatefulInstanceError::ZeroReplicas)
    );
}

#[test]
fn sharded_statefulset_zero_counts_return_error() {
    let cx = cx();
    let sharded = |shard_count: u32, replicas_per_shard: u32| ShardedStatefulSet {
        cx: &cx,
        name: "s",
        component: "server",
        image: "img:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["serve".into()],
        ports: vec![("http", 7000)],
        headless_service: "s-headless",
        shard_count,
        replicas_per_shard,
        voter_count: 3,
        headless_env_key: "SVC_HEADLESS_SERVICE",
        cpu: "1",
        memory: "1Gi",
        extra_env: vec![],
        volume_claim: None,
    };
    assert_eq!(
        sharded_statefulset(sharded(0, 3)),
        Err(StatefulInstanceError::ZeroReplicas)
    );
    assert_eq!(
        sharded_statefulset(sharded(3, 0)),
        Err(StatefulInstanceError::ZeroReplicas)
    );
}

#[test]
fn service_statefulset_with_service_links_zero_replicas_returns_error() {
    let cx = cx();
    let p = ServiceStatefulSet {
        cx: &cx,
        name: "s",
        component: "server",
        image: "img:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["serve".into()],
        args: vec![],
        ports: vec![json!({ "name": "http", "containerPort": 7000, "protocol": "TCP" })],
        headless_service: "s-headless",
        shard_count: 0,
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
    };
    assert_eq!(
        service_statefulset_with_service_links(p, true),
        Err(StatefulInstanceError::ZeroReplicas)
    );
}

fn per_shard<'a>(cx: &'a RenderCtx<'a>, shard_count: u32, replicas: u32) -> ShardedStatefulSet<'a> {
    ShardedStatefulSet {
        cx,
        name: "s",
        component: "server",
        image: "img:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["serve".into()],
        ports: vec![("http", 7000)],
        headless_service: "s-headless",
        shard_count,
        replicas_per_shard: replicas,
        voter_count: replicas,
        headless_env_key: "SVC_HEADLESS_SERVICE",
        cpu: "",
        memory: "",
        extra_env: vec![],
        volume_claim: Some(json!({ "metadata": { "name": "raft" }, "spec": {} })),
    }
}

fn env_value(ss: &Value, key: &str) -> Option<String> {
    ss["spec"]["template"]["spec"]["containers"][0]["env"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == key)
        .and_then(|e| e["value"].as_str().map(str::to_owned))
}

#[test]
fn sharded_statefulsets_one_shard_renders_one_set_with_shard_zero() {
    let cx = cx();
    let sets = sharded_statefulsets(per_shard(&cx, 1, 3)).expect("valid shards");
    assert_eq!(sets.len(), 1);
    let ss = &sets[0];
    assert_eq!(ss["metadata"]["name"], "s");
    assert_eq!(ss["spec"]["replicas"], 3);
    assert_eq!(env_value(ss, "SHARD_ID").as_deref(), Some("0"));
    assert_eq!(env_value(ss, "SHARD_COUNT").as_deref(), Some("1"));
    assert_eq!(ss["spec"]["serviceName"], "s-headless");
    assert_eq!(ss["metadata"]["labels"]["shard"], "0");
    assert_eq!(ss["spec"]["template"]["metadata"]["labels"]["shard"], "0");
    assert_eq!(ss["spec"]["selector"]["matchLabels"]["shard"], "0");
}

#[test]
fn sharded_statefulsets_three_shards_have_own_names_services_ids_and_selectors() {
    let cx = cx();
    let sets = sharded_statefulsets(per_shard(&cx, 3, 3)).expect("valid shards");
    assert_eq!(sets.len(), 3);
    let names: Vec<&str> = sets
        .iter()
        .map(|s| s["metadata"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["s", "s-s1", "s-s2"]);
    let services: Vec<&str> = sets
        .iter()
        .map(|s| s["spec"]["serviceName"].as_str().unwrap())
        .collect();
    assert_eq!(services, ["s-headless", "s-headless-s1", "s-headless-s2"]);
    for (i, ss) in sets.iter().enumerate() {
        let id = i.to_string();
        assert_eq!(env_value(ss, "SHARD_ID").as_deref(), Some(id.as_str()));
        assert_eq!(env_value(ss, "SHARD_COUNT").as_deref(), Some("3"));
        assert_eq!(ss["spec"]["replicas"], 3);
        assert_eq!(ss["spec"]["selector"]["matchLabels"]["shard"], id.as_str());
        assert_eq!(
            env_value(ss, "SVC_HEADLESS_SERVICE").as_deref(),
            ss["spec"]["serviceName"].as_str()
        );
    }
    assert_eq!(
        sets[2]["spec"]["template"]["spec"]["affinity"]["podAntiAffinity"]
            ["requiredDuringSchedulingIgnoredDuringExecution"][0]["labelSelector"]["matchLabels"]
            ["shard"],
        "2"
    );
}

#[test]
fn sharded_statefulsets_pvc_names_keep_shard_zero_and_suffix_the_others() {
    let cx = cx();
    let sets = sharded_statefulsets(per_shard(&cx, 2, 3)).expect("valid shards");
    let pvc_name = |ss: &Value| {
        let claim = ss["spec"]["volumeClaimTemplates"][0]["metadata"]["name"]
            .as_str()
            .unwrap();
        let sts = ss["metadata"]["name"].as_str().unwrap();
        format!("{claim}-{sts}-0")
    };
    assert_eq!(pvc_name(&sets[0]), "raft-s-0");
    assert_eq!(pvc_name(&sets[1]), "raft-s-s1-0");
}

#[test]
fn sharded_statefulsets_zero_counts_return_error() {
    let cx = cx();
    assert!(matches!(
        sharded_statefulsets(per_shard(&cx, 0, 3)),
        Err(StatefulInstanceError::ZeroReplicas)
    ));
    assert!(matches!(
        sharded_statefulsets(per_shard(&cx, 3, 0)),
        Err(StatefulInstanceError::ZeroReplicas)
    ));
}

#[test]
fn sharded_statefulset_single_set_output_is_unchanged() {
    let cx = cx();
    let ss = sharded_statefulset(per_shard(&cx, 2, 3)).expect("valid statefulset");
    assert_eq!(ss["spec"]["replicas"], 6);
    assert_eq!(ss["metadata"]["name"], "s");
    assert_eq!(env_value(&ss, "SHARD_ID"), None);
    assert_eq!(ss["spec"]["selector"]["matchLabels"]["shard"], Value::Null);
}

#[test]
fn sharded_services_render_a_headless_service_per_shard_and_one_client_service() {
    let cx = cx();
    let svcs = sharded_services(&cx, "server", "s-headless", "s-client", 3, 7000).unwrap();
    let names: Vec<&str> = svcs
        .headless
        .iter()
        .map(|s| s["metadata"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["s-headless", "s-headless-s1", "s-headless-s2"]);
    for (i, svc) in svcs.headless.iter().enumerate() {
        assert_eq!(svc["spec"]["clusterIP"], "None");
        assert_eq!(svc["spec"]["selector"]["shard"], i.to_string().as_str());
    }
    let client = &svcs.client;
    assert_eq!(client["metadata"]["name"], "s-client");
    assert_eq!(client["spec"]["type"], "ClusterIP");
    assert!(client["spec"]["selector"].get("shard").is_none());
    assert_eq!(client["spec"]["selector"], cx.selector("server"));
}

#[test]
fn sharded_services_zero_shards_return_error() {
    let cx = cx();
    assert!(matches!(
        sharded_services(&cx, "server", "s-headless", "s-client", 0, 7000),
        Err(StatefulInstanceError::ZeroReplicas)
    ));
}
