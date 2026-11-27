//! Kind NetworkPolicy check (core#652): the same isolation policy as the GKE
//! drill, applied to one pod on the disposable kind cluster of `ci`.

use std::time::Duration;

use serde_json::{json, Value};

use crate::gke_drill_partition::{
    isolation_policy, probe_container, probe_outcome, probe_script, probe_text, wait_until_blocked,
    Probe, CONTROL_POD, ENFORCE_TIMEOUT, ISOLATED_POD, POD_NAME_LABEL, POLICY_NAME, PROBE_IMAGE,
};
use crate::probe_pod_kind::on_kind_cluster;

const KIND_NAMESPACE: &str = "netpol-kind";

/// The GKE isolation policy, in the kind namespace (only the namespace differs).
pub fn kind_policy() -> Value {
    let mut policy = isolation_policy();
    policy["metadata"]["namespace"] = json!(KIND_NAMESPACE);
    policy
}

/// A pod with the labels that the policy selects; it listens on no port.
pub fn kind_pod(name: &str) -> Value {
    let mut labels = isolation_policy()["spec"]["podSelector"]["matchLabels"].clone();
    labels[POD_NAME_LABEL] = json!(name);
    json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {"name": name, "namespace": KIND_NAMESPACE, "labels": labels},
        "spec": {
            "containers": [{
                "name": "main",
                "image": PROBE_IMAGE,
                "command": ["sleep", "3600"],
                "resources": {"requests": {"cpu": "10m", "memory": "16Mi"}}
            }],
            "terminationGracePeriodSeconds": 0
        }
    })
}

/// The overrides of a probe pod. The kind cluster has no client pool, so no node selector.
pub fn kind_probe_overrides(name: &str, script: &str) -> Value {
    json!({"spec": {"containers": [probe_container(name, script)]}})
}

/// Deletes the namespace on every exit path, so a failed run leaves no pod or policy behind.
struct NamespaceGuard<'a> {
    kube: &'a e2e_support::Kube,
}

impl Drop for NamespaceGuard<'_> {
    fn drop(&mut self) {
        let _ = self.kube.try_run(&[
            "delete",
            "namespace",
            KIND_NAMESPACE,
            "--ignore-not-found",
            "--wait=true",
        ]);
    }
}

/// Writes the object to a file in the temporary directory and applies it.
fn apply_json(kube: &e2e_support::Kube, name: &str, object: &Value) {
    let path = std::env::temp_dir().join(format!("{name}.json"));
    std::fs::write(
        &path,
        serde_json::to_string(object).expect("the object serializes"),
    )
    .expect("the manifest is written");
    let path = path.to_string_lossy().to_string();
    kube.run(&["apply", "-f", &path]);
}

fn pod_ip(kube: &e2e_support::Kube, name: &str) -> String {
    kube.get(&format!("pod/{name}"))["status"]["podIP"]
        .as_str()
        .expect("the pod has an IP")
        .to_string()
}

/// Runs one probe pod against `ip` and returns what it saw.
fn probe(kube: &e2e_support::Kube, label: &str, ip: &str) -> Probe {
    let name = format!("probe-{label}");
    let script = probe_script(ip);
    kube.run(&[
        "run",
        &name,
        "--restart=Never",
        "--image",
        PROBE_IMAGE,
        "--overrides",
        &kind_probe_overrides(&name, &script).to_string(),
    ]);
    kube.run(&[
        "wait",
        "--for=jsonpath={.status.phase}=Succeeded",
        &format!("pod/{name}"),
        "--timeout=180s",
    ]);
    let text = e2e_support::wait_for("probe text", Duration::from_secs(30), || {
        let logs = kube.run(&["logs", &name]);
        let pod = kube.get(&format!("pod/{name}"));
        let message = pod["status"]["containerStatuses"][0]["state"]["terminated"]["message"]
            .as_str()
            .unwrap_or_default();
        probe_text(&logs, message)
    });
    kube.run(&["delete", "pod", &name, "--wait=true"]);
    e2e_support::say(format!("probe-{label} output: {}", text.trim()));
    probe_outcome(&text).unwrap_or_else(|e| panic!("probe-{label}: {e}"))
}

#[test]
fn kind_policy_keeps_the_gke_policy_spec() {
    let kind = kind_policy();
    let gke = isolation_policy();
    assert_eq!(kind["spec"], gke["spec"]);
    assert_eq!(kind["metadata"]["namespace"], KIND_NAMESPACE);
    assert_eq!(kind["metadata"]["name"], POLICY_NAME);
}

#[test]
fn kind_pod_of_the_isolated_name_matches_the_policy_selector() {
    let policy = kind_policy();
    let pod = kind_pod(ISOLATED_POD);
    for (key, value) in policy["spec"]["podSelector"]["matchLabels"]
        .as_object()
        .expect("the selector is a map")
    {
        assert_eq!(
            &pod["metadata"]["labels"][key.as_str()],
            value,
            "label {key}"
        );
    }
}

#[test]
fn kind_pod_of_the_control_name_is_not_selected_by_the_policy() {
    let policy = kind_policy();
    let control = kind_pod(CONTROL_POD);
    assert_eq!(
        control["metadata"]["labels"][POD_NAME_LABEL],
        json!(CONTROL_POD)
    );
    let selected = policy["spec"]["podSelector"]["matchLabels"]
        .as_object()
        .expect("the selector is a map")
        .iter()
        .all(|(key, value)| &control["metadata"]["labels"][key.as_str()] == value);
    assert!(!selected, "the policy must not select {CONTROL_POD}");
}

#[test]
fn kind_probe_overrides_have_no_pool_selector() {
    let script = probe_script("10.0.0.5");
    let overrides = kind_probe_overrides("probe-x", &script);
    assert!(overrides["spec"]["nodeSelector"].is_null());
    assert!(overrides["spec"]["tolerations"].is_null());
    assert_eq!(overrides["spec"]["containers"][0]["name"], "probe-x");
    assert_eq!(
        overrides["spec"]["containers"][0]["command"][2],
        json!(probe_script("10.0.0.5"))
    );
}

#[test]
fn kind_pod_has_a_sleep_command_and_no_listener() {
    let pod = kind_pod("drill-0");
    assert_eq!(
        pod["spec"]["containers"][0]["command"],
        json!(["sleep", "3600"])
    );
    assert_eq!(pod["spec"]["containers"][0]["image"], PROBE_IMAGE);
}

#[test]
#[ignore = "live kind cluster: needs SERVICE_K8S_LIVE_KUBE=1; runs in CI job `e2e: service-k8s kind`"]
fn kind_policy_isolates_one_pod_and_heals_when_it_is_removed() {
    if std::env::var("SERVICE_K8S_LIVE_KUBE").unwrap_or_default() != "1" {
        eprintln!(
            "skip kind_policy_isolates_one_pod_and_heals_when_it_is_removed: SERVICE_K8S_LIVE_KUBE=1 is not set"
        );
        return;
    }
    if !on_kind_cluster() {
        eprintln!(
            "skip kind_policy_isolates_one_pod_and_heals_when_it_is_removed: the current context is not a kind cluster"
        );
        return;
    }

    let kube = e2e_support::Kube {
        namespace: KIND_NAMESPACE.to_string(),
    };
    let _ = kube.try_run(&[
        "delete",
        "namespace",
        KIND_NAMESPACE,
        "--ignore-not-found",
        "--wait=true",
    ]);
    kube.run(&["create", "namespace", KIND_NAMESPACE]);
    let _guard = NamespaceGuard { kube: &kube };

    apply_json(&kube, ISOLATED_POD, &kind_pod(ISOLATED_POD));
    apply_json(&kube, CONTROL_POD, &kind_pod(CONTROL_POD));
    let isolated_pod = format!("pod/{ISOLATED_POD}");
    let control_pod = format!("pod/{CONTROL_POD}");
    kube.run(&[
        "wait",
        "--for=condition=Ready",
        &isolated_pod,
        &control_pod,
        "--timeout=180s",
    ]);
    let isolated_ip = pod_ip(&kube, ISOLATED_POD);
    let control_ip = pod_ip(&kube, CONTROL_POD);

    assert_eq!(
        probe(&kube, "before", &isolated_ip),
        Probe::Reachable,
        "drill-0 is not reachable before the policy"
    );

    apply_json(&kube, POLICY_NAME, &kind_policy());
    let probes = wait_until_blocked(ENFORCE_TIMEOUT, Duration::from_secs(2), || {
        probe(&kube, "isolated", &isolated_ip)
    })
    .unwrap_or_else(|e| {
        panic!(
            "kind cluster does not enforce NetworkPolicy: drill-0 is still reachable under the policy: {e}"
        )
    });
    e2e_support::say(format!("drill-0 blocked after {probes} probe(s)"));

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
}
