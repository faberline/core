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

/// Runs wget without `-q` and ends with a `wget-exit=<code>` marker line, so the log is never empty.
pub fn probe_command(ip: &str) -> String {
    format!("wget -T 3 -O- http://{ip}:8080 2>&1; echo \"wget-exit=$?\"")
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

/// Runs the probe and also writes its result to the termination log, so the result survives an empty log read.
fn probe_script(ip: &str) -> String {
    format!(
        "{{ {}; }} 2>&1 | tee /dev/termination-log",
        probe_command(ip)
    )
}

/// The termination message of the probe container, when it has one.
fn terminated_message(pod: &Value) -> Option<String> {
    pod["status"]["containerStatuses"][0]["state"]["terminated"]["message"]
        .as_str()
        .map(str::to_string)
}

/// The probe's text: the logs when they are not empty, else the termination message.
fn probe_text(logs: &str, termination_message: &str) -> Result<String, String> {
    nonempty_logs(logs)
        .or_else(|_| nonempty_logs(termination_message))
        .map_err(|_| "probe logs and termination message are empty".to_string())
}

/// The evidence for a probe that got no text: the log call, the pod and its events.
fn empty_probe_diagnostic(pod: &Value, raw_log: &Result<String, String>, events: &str) -> String {
    let log_line = match raw_log {
        Ok(body) => format!("log call: ok, body length {}", body.len()),
        Err(e) => format!("log call: error: {e}"),
    };
    let uid = pod["metadata"]["uid"].as_str().unwrap_or("unknown");
    let terminated = &pod["status"]["containerStatuses"][0]["state"]["terminated"];
    let exit_code = terminated["exitCode"]
        .as_i64()
        .map_or("unknown".to_string(), |code| code.to_string());
    let reason = terminated["reason"].as_str().unwrap_or("unknown");
    let message = terminated["message"].as_str().unwrap_or("unknown");
    format!(
        "{log_line}\npod uid: {uid}\nterminated: exit code {exit_code}, reason {reason}, message {message}\nevents:\n{events}"
    )
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
        &probe_script(ip),
    ]);
    kube.run(&[
        "wait",
        "--for=jsonpath={.status.phase}=Succeeded",
        &format!("pod/{name}"),
        "--timeout=600s",
    ]);
    let deadline = std::time::Instant::now() + LOGS_RETRY_TIMEOUT;
    let text = loop {
        let logs = kube.run(&["logs", &name]);
        let pod = kube.get(&format!("pod/{name}"));
        match probe_text(&logs, &terminated_message(&pod).unwrap_or_default()) {
            Ok(text) => break text,
            Err(_) if std::time::Instant::now() >= deadline => {
                let raw_log = kube.try_run(&[
                    "get",
                    "--raw",
                    &format!("/api/v1/namespaces/{}/pods/{name}/log", kube.namespace),
                ]);
                let events = kube
                    .try_run(&[
                        "get",
                        "events",
                        "--field-selector",
                        &format!("involvedObject.name={name}"),
                    ])
                    .unwrap_or_else(|e| e);
                let phase = pod["status"]["phase"].as_str().unwrap_or("unknown");
                let diagnostic = empty_probe_diagnostic(&pod, &raw_log, &events);
                let _ = kube.try_run(&["delete", "pod", &name, "--wait=true"]);
                panic!(
                    "{}",
                    probe_failure(
                        label,
                        phase,
                        &format!("logs and termination message stayed empty for 30s\n{diagnostic}"),
                    )
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
        "wget -T 3 -O- http://10.0.0.5:8080 2>&1; echo \"wget-exit=$?\""
    );
}

#[test]
fn probe_command_is_not_quiet_and_ends_with_an_exit_marker() {
    let cmd = probe_command("10.0.0.5");
    assert!(!cmd.contains(" -q "), "{cmd}");
    assert!(cmd.contains("echo \"wget-exit=$?\""), "{cmd}");
}

#[test]
fn probe_outcome_blocked_on_timeout_with_marker() {
    assert_eq!(
        probe_outcome(
            "Connecting to 10.0.0.5:8080 (10.0.0.5:8080)\nwget: download timed out\nwget-exit=1\n"
        ),
        Ok(Probe::Blocked)
    );
}

#[test]
fn probe_outcome_reachable_on_refusal_with_marker() {
    assert_eq!(
        probe_outcome(
            "Connecting to 10.0.0.5:8080 (10.0.0.5:8080)\nwget: can't connect to remote host (10.0.0.5): Connection refused\nwget-exit=1\n"
        ),
        Ok(Probe::Reachable)
    );
}

#[test]
fn probe_outcome_rejects_marker_only_text() {
    let err = probe_outcome("wget-exit=1\n").expect_err("a marker alone is not a probe result");
    assert!(err.contains("unexpected probe output"), "{err}");
    assert!(err.contains("wget-exit=1"), "{err}");
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

fn terminated_pod(message: &str) -> Value {
    json!({
        "metadata": { "uid": "abc-123" },
        "status": {
            "phase": "Succeeded",
            "containerStatuses": [{
                "state": { "terminated": {
                    "exitCode": 0, "reason": "Completed", "message": message
                } }
            }]
        }
    })
}

#[test]
fn probe_script_tees_the_result_to_the_termination_log() {
    let script = probe_script("10.0.0.5");
    assert_eq!(
        script,
        "{ wget -T 3 -O- http://10.0.0.5:8080 2>&1; echo \"wget-exit=$?\"; } 2>&1 | tee /dev/termination-log"
    );
    assert!(script.contains(&probe_command("10.0.0.5")), "{script}");
}

#[test]
fn terminated_message_reads_the_container_status() {
    assert_eq!(
        terminated_message(&terminated_pod("wget-exit=1\n")),
        Some("wget-exit=1\n".to_string())
    );
}

#[test]
fn terminated_message_is_none_without_a_terminated_state() {
    assert_eq!(
        terminated_message(&json!({"status": {"containerStatuses": [{"state": {"running": {}}}]}})),
        None
    );
    assert_eq!(terminated_message(&json!({})), None);
}

#[test]
fn probe_text_prefers_the_logs() {
    assert_eq!(
        probe_text("wget: download timed out\n", "wget-exit=1\n"),
        Ok("wget: download timed out\n".to_string())
    );
}

#[test]
fn probe_text_falls_back_to_the_termination_message() {
    assert_eq!(
        probe_text("", "wget: download timed out\nwget-exit=1\n"),
        Ok("wget: download timed out\nwget-exit=1\n".to_string())
    );
    assert_eq!(probe_text("  \n", "x"), Ok("x".to_string()));
}

#[test]
fn probe_text_fails_when_both_are_empty() {
    assert_eq!(
        probe_text("", " \n"),
        Err("probe logs and termination message are empty".to_string())
    );
}

#[test]
fn probe_text_feeds_probe_outcome() {
    let message = terminated_message(&terminated_pod(
        "Connecting to 10.0.0.5:8080\nwget: download timed out\nwget-exit=1\n",
    ))
    .expect("the terminated container has a message");
    let text = probe_text("", &message).expect("the termination message is used");
    assert_eq!(probe_outcome(&text), Ok(Probe::Blocked));
}

#[test]
fn empty_probe_diagnostic_names_every_field() {
    let text = empty_probe_diagnostic(
        &terminated_pod(""),
        &Ok(String::new()),
        "Normal Started pod/probe-before",
    );
    assert!(text.contains("log call: ok, body length 0"), "{text}");
    assert!(text.contains("pod uid: abc-123"), "{text}");
    assert!(
        text.contains("terminated: exit code 0, reason Completed, message "),
        "{text}"
    );
    assert!(
        text.contains("events:\nNormal Started pod/probe-before"),
        "{text}"
    );
}

#[test]
fn empty_probe_diagnostic_reports_a_failed_log_call_and_missing_fields() {
    let text = empty_probe_diagnostic(&json!({}), &Err("Forbidden".to_string()), "");
    assert!(text.contains("log call: error: Forbidden"), "{text}");
    assert!(text.contains("pod uid: unknown"), "{text}");
    assert!(
        text.contains("terminated: exit code unknown, reason unknown, message unknown"),
        "{text}"
    );
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
