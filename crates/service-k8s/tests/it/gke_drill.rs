//! Harness for the core drills on the shared GKE test cluster (core#379).
//!
//! The drills check only what core controls: placement, rescheduling,
//! NetworkPolicy, rollout order and status conditions. The pods run a no-op
//! image, so no raft traffic is tested here.

use std::time::Duration;

use serde_json::{json, Value};
use service_k8s::render::{
    dedicated_node_affinity, headless_service_with_ports, requested_resources, service_statefulset,
    RenderCtx, ServiceStatefulSet,
};

pub const POOL_KEY: &str = "faberline.dev/pool";
pub const POOL_SERVER: &str = "core-server";
pub const PULL_SECRET: &str = "ghcr-pull";
pub const DRILL_NAME: &str = "drill";
pub const DRILL_NAMESPACE: &str = "core";
pub const DRILL_REPLICAS: u32 = 3;
pub const OVERLAY_FILE: &str = "k8s/overlays/gke-test/resources.yaml";
/// Label selector for the drill pods.
pub const DRILL_POD_LABEL: &str =
    "app.kubernetes.io/instance=drill,app.kubernetes.io/component=server";

/// The StatefulSet and the headless Service that the overlay holds, as JSON.
pub fn render_drill_objects() -> Vec<Value> {
    let cx = RenderCtx::new(
        "core-drill",
        "core-drill",
        "core.faberline.dev/v1",
        "DrillInstance",
        DRILL_NAME,
        DRILL_NAMESPACE,
    );
    let mut statefulset = service_statefulset(ServiceStatefulSet {
        cx: &cx,
        name: DRILL_NAME,
        component: "server",
        image: "core",
        image_pull_policy: "IfNotPresent",
        command: vec![],
        args: vec![],
        ports: vec![json!({"name": "http", "containerPort": 8080})],
        headless_service: "drill-headless",
        shard_count: 1,
        replicas_per_shard: DRILL_REPLICAS,
        voter_count: DRILL_REPLICAS,
        headless_env_key: "DRILL_HEADLESS_SERVICE",
        service_account_name: None,
        env: vec![],
        env_from: vec![],
        resources: requested_resources("100m", "128Mi"),
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
        affinity: Some(dedicated_node_affinity(cx.selector("server"))),
        node_selector: Some(json!({ POOL_KEY: POOL_SERVER })),
        tolerations: vec![json!({
            "key": POOL_KEY,
            "operator": "Equal",
            "value": POOL_SERVER,
            "effect": "NoSchedule"
        })],
        topology_spread_constraints: vec![],
        revision_history_limit: None,
        update_strategy: None,
        volume_claim: None,
    })
    .expect("drill StatefulSet renders");
    statefulset["spec"]["template"]["spec"]["imagePullSecrets"] = json!([{ "name": PULL_SECRET }]);
    let service = headless_service_with_ports(
        &cx,
        "drill-headless",
        "server",
        vec![json!({"name": "http", "port": 8080, "targetPort": "http", "protocol": "TCP"})],
    );
    vec![statefulset, service]
}

/// The overlay file text: each object as YAML, joined by `---\n`.
pub fn overlay_yaml() -> String {
    render_drill_objects()
        .iter()
        .map(|obj| serde_yaml::to_string(obj).expect("object serializes as YAML"))
        .collect::<Vec<_>>()
        .join("---\n")
}

/// The name of the first drill pod that is Running and Ready.
/// Returns `Err(text)` when no drill pod is ready (for use inside `wait_for`).
pub fn ready_pod(pods: &Value) -> Result<String, String> {
    let items = pods["items"].as_array().cloned().unwrap_or_default();
    items
        .iter()
        .find(|pod| {
            pod["status"]["phase"] == "Running"
                && pod["status"]["conditions"].as_array().is_some_and(|conds| {
                    conds
                        .iter()
                        .any(|c| c["type"] == "Ready" && c["status"] == "True")
                })
        })
        .and_then(|pod| pod["metadata"]["name"].as_str().map(str::to_string))
        .ok_or_else(|| format!("no ready pod among {}", items.len()))
}

fn overlay_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(OVERLAY_FILE)
}

fn pod_with_ready(name: &str, phase: &str, ready: Option<&str>) -> Value {
    let conditions = match ready {
        Some(status) => json!([{ "type": "Ready", "status": status }]),
        None => json!([]),
    };
    json!({
        "metadata": { "name": name },
        "status": { "phase": phase, "conditions": conditions }
    })
}

#[test]
fn overlay_file_matches_the_render() {
    let want = overlay_yaml();
    let path = overlay_path();
    if std::env::var("UPDATE_GKE_OVERLAY").as_deref() == Ok("1") {
        std::fs::create_dir_all(path.parent().expect("overlay has a directory"))
            .expect("overlay directory is created");
        std::fs::write(&path, &want).expect("overlay file is written");
        return;
    }
    let got = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        got, want,
        "run: UPDATE_GKE_OVERLAY=1 cargo test -p service-k8s --test it gke_drill::overlay_file_matches_the_render"
    );
}

#[test]
fn drill_statefulset_obeys_the_gke_pool_rules() {
    let objects = render_drill_objects();
    let statefulset = &objects[0];
    assert_eq!(statefulset["spec"]["replicas"], 3);

    let pod_spec = &statefulset["spec"]["template"]["spec"];
    assert_eq!(pod_spec["nodeSelector"][POOL_KEY], POOL_SERVER);
    assert_eq!(
        pod_spec["tolerations"],
        json!([{
            "key": POOL_KEY,
            "operator": "Equal",
            "value": POOL_SERVER,
            "effect": "NoSchedule"
        }])
    );
    assert_eq!(pod_spec["imagePullSecrets"][0]["name"], PULL_SECRET);

    let container = &pod_spec["containers"][0];
    assert!(container["resources"]["requests"]["cpu"].is_string());
    assert!(container["resources"]["requests"]["memory"].is_string());
    assert_eq!(container["image"], "core");
    assert_eq!(
        pod_spec["affinity"]["podAntiAffinity"]["requiredDuringSchedulingIgnoredDuringExecution"]
            [0]["topologyKey"],
        "kubernetes.io/hostname"
    );

    let placement = e2e_support::PoolPlacement {
        key: POOL_KEY.into(),
        value: POOL_SERVER.into(),
        pull_secret: PULL_SECRET.into(),
    };
    placement.assert_pool_placement("drill StatefulSet", statefulset);

    // Negative control: without the nodeSelector the placement check must fail.
    let mut without_selector = statefulset.clone();
    without_selector["spec"]["template"]["spec"]
        .as_object_mut()
        .expect("pod spec is an object")
        .remove("nodeSelector");
    assert!(placement
        .workload_placement_error("drill", &without_selector)
        .is_some());
}

#[test]
fn drill_service_is_headless() {
    let objects = render_drill_objects();
    let service = &objects[1];
    assert_eq!(service["spec"]["clusterIP"], "None");
    assert_eq!(service["metadata"]["namespace"], DRILL_NAMESPACE);
}

#[test]
fn ready_pod_picks_the_first_ready_pod() {
    let pods = json!({
        "items": [
            pod_with_ready("drill-0", "Pending", None),
            pod_with_ready("drill-1", "Running", Some("False")),
            pod_with_ready("drill-2", "Running", Some("True")),
        ]
    });
    assert_eq!(ready_pod(&pods), Ok("drill-2".to_string()));
}

#[test]
fn ready_pod_reports_when_none_is_ready() {
    let pods = json!({
        "items": [
            pod_with_ready("drill-0", "Pending", None),
            pod_with_ready("drill-1", "Running", Some("False")),
        ]
    });
    assert_eq!(ready_pod(&pods), Err("no ready pod among 2".to_string()));
}

#[test]
#[ignore = "needs the shared GKE cluster: run by .github/workflows/gke-drill.yml (core#379), which sets KUBECONFIG and NAMESPACE"]
fn smoke_drill_statefulset_is_ready_on_the_core_server_pool() {
    let kube = e2e_support::Kube::from_env();
    let _dump = e2e_support::DumpOnFailure {
        suite: "gke-drill",
        kube: &kube,
        dump_commands: vec![vec![
            "get".into(),
            "pods".into(),
            "-o".into(),
            "wide".into(),
        ]],
    };
    let step = e2e_support::Step::begin("gke-drill", "smoke");

    e2e_support::wait_ready(&kube, DRILL_POD_LABEL, 3, Duration::from_secs(600));
    let pods = kube.pods(DRILL_POD_LABEL);
    assert!(ready_pod(&pods).is_ok(), "no ready drill pod");

    let placement = e2e_support::PoolPlacement {
        key: POOL_KEY.into(),
        value: POOL_SERVER.into(),
        pull_secret: PULL_SECRET.into(),
    };
    let items = pods["items"].as_array().cloned().unwrap_or_default();
    let mut nodes: Vec<String> = Vec::new();
    let mut lines: Vec<Value> = Vec::new();
    for pod in &items {
        placement.assert_on_pool(pod);
        let name = pod["metadata"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let node = pod["spec"]["nodeName"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        e2e_support::say(format!("{name} on {node}"));
        lines.push(json!({ "pod": name, "node": node }));
        nodes.push(node);
    }
    nodes.sort();
    nodes.dedup();
    assert_eq!(nodes.len(), items.len(), "drill pods share a node");

    e2e_support::evidence(
        "gke-drill",
        "smoke",
        json!({ "pods": lines, "nodes": nodes }),
    );
    step.end();
}
