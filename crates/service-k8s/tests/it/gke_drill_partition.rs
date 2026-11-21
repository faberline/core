//! Partition drill (core#381): a NetworkPolicy isolates `drill-0`, a probe pod
//! shows that the isolation is real, the policy is removed and `drill-0` is
//! reachable again. It checks no write: acknowledged writes are in core#361.

use std::time::Duration;

use serde_json::{json, Value};
use service_k8s::render::common::{network_policy, NetworkPolicy};
use service_k8s::render::RenderCtx;

use crate::gke_drill::{
    on_gke_drill_cluster, DRILL_NAME, DRILL_NAMESPACE, DRILL_POD_LABEL, POOL_KEY,
};

const ISOLATED_POD: &str = "drill-0";
const CONTROL_POD: &str = "drill-1";
const POLICY_NAME: &str = "drill-isolate-0";
const PROBE_IMAGE: &str = "busybox:1.36.1";
const CLIENT_POOL: &str = "core-client";
const POD_NAME_LABEL: &str = "statefulset.kubernetes.io/pod-name";

#[derive(Debug, PartialEq, Eq)]
pub enum Probe {
    Reachable,
    Blocked,
}

/// The NetworkPolicy that selects only `drill-0` and denies all its traffic.
pub fn isolation_policy() -> Value {
    let cx = RenderCtx::new(
        "core-drill",
        "core-drill",
        "core.faberline.dev/v1",
        "DrillInstance",
        DRILL_NAME,
        DRILL_NAMESPACE,
    );
    let mut policy = network_policy(NetworkPolicy {
        cx: &cx,
        name: POLICY_NAME,
        component: "server",
        client_ports: vec![],
        peer_ports: vec![],
        extra_egress: vec![],
    });
    policy["spec"]["podSelector"]["matchLabels"][POD_NAME_LABEL] = json!(ISOLATED_POD);
    policy["spec"]["ingress"] = json!([]);
    policy["spec"]["egress"] = json!([]);
    policy
}

/// Classifies the probe's output: a timeout is blocked, a refusal is reachable.
pub fn probe_outcome(text: &str) -> Result<Probe, String> {
    if text.contains("timed out") {
        Ok(Probe::Blocked)
    } else if text.contains("Connection refused") {
        Ok(Probe::Reachable)
    } else {
        Err(format!("unexpected probe output: {text}"))
    }
}

pub fn probe_command(ip: &str) -> String {
    format!("wget -T 3 -q -O- http://{ip}:8080 2>&1; true")
}

const LOGS_RETRY_TIMEOUT: Duration = Duration::from_secs(30);

/// Empty or blank logs are not a probe result yet: the caller retries.
fn nonempty_logs(text: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        Err("probe logs are empty".to_string())
    } else {
        Ok(text.to_string())
    }
}

/// The panic text for a probe: it names the probe and the pod phase.
fn probe_failure(label: &str, phase: &str, error: &str) -> String {
    format!("probe-{label} (pod phase {phase}): {error}")
}

/// Runs the one-off probe pod on the client pool; its container is named `name`.
pub fn probe_overrides(name: &str) -> Value {
    json!({"spec": {
        "nodeSelector": { POOL_KEY: CLIENT_POOL },
        "tolerations": [{
            "key": POOL_KEY,
            "operator": "Equal",
            "value": CLIENT_POOL,
            "effect": "NoSchedule"
        }],
        "containers": [{
            "name": name,
            "image": PROBE_IMAGE,
            "resources": { "requests": { "cpu": "50m", "memory": "32Mi" } }
        }]
    }})
}

fn pod_ip(pods: &Value, name: &str) -> Option<String> {
    pods["items"]
        .as_array()?
        .iter()
        .find(|pod| pod["metadata"]["name"] == name)
        .and_then(|pod| pod["status"]["podIP"].as_str().map(str::to_string))
}

/// Deletes the policy on every exit path, so a failed run leaves none behind.
struct PolicyGuard<'a> {
    kube: &'a e2e_support::Kube,
}

impl Drop for PolicyGuard<'_> {
    fn drop(&mut self) {
        let _ = self
            .kube
            .try_run(&["delete", "networkpolicy", POLICY_NAME, "--ignore-not-found"]);
    }
}

fn probe(kube: &e2e_support::Kube, label: &str, ip: &str) -> Probe {
    let name = format!("probe-{label}");
    kube.run(&[
        "run",
        &name,
        "--restart=Never",
        "--image",
        PROBE_IMAGE,
        "--overrides",
        &probe_overrides(&name).to_string(),
        "--command",
        "--",
        "sh",
        "-c",
        &probe_command(ip),
    ]);
    kube.run(&[
        "wait",
        "--for=jsonpath={.status.phase}=Succeeded",
        &format!("pod/{name}"),
        "--timeout=600s",
    ]);
    let deadline = std::time::Instant::now() + LOGS_RETRY_TIMEOUT;
    let text = loop {
        match nonempty_logs(&kube.run(&["logs", &name])) {
            Ok(text) => break text,
            Err(_) if std::time::Instant::now() >= deadline => {
                let phase = kube.run(&[
                    "get",
                    &format!("pod/{name}"),
                    "-o",
                    "jsonpath={.status.phase}",
                ]);
                let _ = kube.try_run(&["delete", "pod", &name, "--wait=true"]);
                panic!(
                    "{}",
                    probe_failure(label, &phase, "logs stayed empty for 30s")
                );
            }
            Err(_) => std::thread::sleep(Duration::from_secs(2)),
        }
    };
    kube.run(&["delete", "pod", &name, "--wait=true"]);
    probe_outcome(&text).unwrap_or_else(|e| panic!("{}", probe_failure(label, "Succeeded", &e)))
}

#[test]
fn isolation_policy_selects_one_pod_and_denies_everything() {
    let policy = isolation_policy();
    assert_eq!(policy["kind"], "NetworkPolicy");
    assert_eq!(policy["metadata"]["name"], POLICY_NAME);
    assert_eq!(policy["metadata"]["namespace"], DRILL_NAMESPACE);
    assert_eq!(
        policy["spec"]["podSelector"]["matchLabels"][POD_NAME_LABEL],
        ISOLATED_POD
    );
    assert_eq!(policy["spec"]["policyTypes"], json!(["Ingress", "Egress"]));
    assert_eq!(policy["spec"]["ingress"], json!([]));
    assert_eq!(policy["spec"]["egress"], json!([]));
}

#[test]
fn isolation_policy_keeps_the_instance_selector() {
    let policy = isolation_policy();
    assert_eq!(
        policy["spec"]["podSelector"]["matchLabels"]["app.kubernetes.io/instance"],
        "drill"
    );
}

#[test]
fn probe_outcome_blocked_on_timeout() {
    assert_eq!(
        probe_outcome("wget: download timed out"),
        Ok(Probe::Blocked)
    );
}

#[test]
fn probe_outcome_reachable_on_refusal() {
    assert_eq!(
        probe_outcome("wget: can't connect to remote host (10.0.0.5): Connection refused"),
        Ok(Probe::Reachable)
    );
}

#[test]
fn probe_outcome_rejects_unknown_text() {
    let err = probe_outcome("").expect_err("empty output is not a probe result");
    assert!(err.contains("unexpected probe output"), "{err}");
}

#[test]
fn probe_command_targets_port_8080() {
    assert_eq!(
        probe_command("10.0.0.5"),
        "wget -T 3 -q -O- http://10.0.0.5:8080 2>&1; true"
    );
}

#[test]
fn probe_overrides_run_on_the_client_pool() {
    let overrides = probe_overrides("probe-x");
    assert_eq!(overrides["spec"]["nodeSelector"][POOL_KEY], CLIENT_POOL);
    assert_eq!(overrides["spec"]["tolerations"][0]["value"], CLIENT_POOL);
    assert_eq!(overrides["spec"]["containers"][0]["name"], "probe-x");
}

#[test]
fn nonempty_logs_rejects_empty_and_blank_text() {
    assert_eq!(nonempty_logs(""), Err("probe logs are empty".to_string()));
    assert_eq!(
        nonempty_logs("  \n"),
        Err("probe logs are empty".to_string())
    );
}

#[test]
fn nonempty_logs_passes_text_through() {
    assert_eq!(
        nonempty_logs("wget: download timed out\n"),
        Ok("wget: download timed out\n".to_string())
    );
}

#[test]
fn probe_failure_names_the_label_and_the_phase() {
    assert_eq!(
        probe_failure("isolated", "Succeeded", "unexpected probe output: "),
        "probe-isolated (pod phase Succeeded): unexpected probe output: "
    );
}

#[test]
fn pod_ip_finds_the_named_pod() {
    let pods = json!({
        "items": [
            { "metadata": { "name": "drill-0" }, "status": { "podIP": "10.0.0.1" } },
            { "metadata": { "name": "drill-1" }, "status": { "podIP": "10.0.0.2" } },
        ]
    });
    assert_eq!(pod_ip(&pods, "drill-1"), Some("10.0.0.2".to_string()));
    assert_eq!(pod_ip(&pods, "drill-9"), None);
}

#[test]
#[ignore = "needs the shared GKE cluster: run by .github/workflows/gke-drill.yml (core#381), which sets GKE_DRILL=1, KUBECONFIG and NAMESPACE"]
fn partition_isolates_one_pod_and_heals_when_the_policy_is_removed() {
    if !on_gke_drill_cluster("partition_isolates_one_pod_and_heals_when_the_policy_is_removed") {
        return;
    }
    let kube = e2e_support::Kube::from_env();
    let _dump = e2e_support::DumpOnFailure {
        suite: "gke-drill",
        kube: &kube,
        dump_commands: vec![
            vec!["get".into(), "pods".into(), "-o".into(), "wide".into()],
            vec![
                "get".into(),
                "networkpolicy".into(),
                "-o".into(),
                "yaml".into(),
            ],
        ],
    };
    let step = e2e_support::Step::begin("gke-drill", "partition");

    e2e_support::wait_ready(&kube, DRILL_POD_LABEL, 3, Duration::from_secs(600));
    let pods = kube.pods(DRILL_POD_LABEL);
    let isolated_ip = pod_ip(&pods, ISOLATED_POD).expect("drill-0 has an IP");
    let control_ip = pod_ip(&pods, CONTROL_POD).expect("drill-1 has an IP");

    assert_eq!(
        probe(&kube, "before", &isolated_ip),
        Probe::Reachable,
        "drill-0 is not reachable before the policy"
    );

    let policy_path = std::env::temp_dir().join("drill-isolate-0.yaml");
    std::fs::write(
        &policy_path,
        serde_yaml::to_string(&isolation_policy()).expect("policy serializes as YAML"),
    )
    .expect("policy file is written");
    kube.run(&[
        "apply",
        "-f",
        policy_path.to_str().expect("temp path is UTF-8"),
    ]);
    let _guard = PolicyGuard { kube: &kube };

    assert_eq!(
        probe(&kube, "isolated", &isolated_ip),
        Probe::Blocked,
        "drill-0 is still reachable under the policy"
    );
    assert_eq!(
        probe(&kube, "control", &control_ip),
        Probe::Reachable,
        "drill-1 must stay reachable"
    );

    kube.run(&["delete", "networkpolicy", POLICY_NAME, "--wait=true"]);

    e2e_support::wait_for(
        "drill-0 reachable again",
        Duration::from_secs(120),
        || match probe(&kube, "after", &isolated_ip) {
            Probe::Reachable => Ok(()),
            Probe::Blocked => Err("still blocked".to_string()),
        },
    );

    e2e_support::wait_ready(&kube, DRILL_POD_LABEL, 3, Duration::from_secs(120));

    e2e_support::evidence(
        "gke-drill",
        "partition",
        json!({
            "isolated": ISOLATED_POD,
            "control": CONTROL_POD,
            "before": "reachable",
            "during": "blocked",
            "control_during": "reachable",
            "after": "reachable"
        }),
    );
    step.end();
}
