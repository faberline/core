//! Probe pod on kind (core#629): the probe container prints its marker line
//! and writes its result to the termination message.

use serde_json::json;

use crate::gke_drill_partition::{probe_container, probe_outcome, probe_script, probe_text, Probe};

const PROBE_IMAGE: &str = "busybox:1.36.1";

/// Whether the current kubectl context is a kind cluster.
fn on_kind_cluster() -> bool {
    std::process::Command::new("kubectl")
        .args(["config", "current-context"])
        .output()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .starts_with("kind-")
        })
        .unwrap_or(false)
}

/// Deletes the probe pod when it goes out of scope, so a failed run leaves none behind.
struct PodGuard<'a> {
    kube: &'a e2e_support::Kube,
    name: String,
}

impl Drop for PodGuard<'_> {
    fn drop(&mut self) {
        let _ = self
            .kube
            .try_run(&["delete", "pod", &self.name, "--wait=true"]);
    }
}

#[test]
#[ignore = "live kind cluster: needs SERVICE_K8S_LIVE_KUBE=1; runs in CI job `e2e: service-k8s kind`"]
fn probe_pod_prints_its_marker_and_termination_message() {
    if std::env::var("SERVICE_K8S_LIVE_KUBE").unwrap_or_default() != "1" {
        eprintln!(
            "skip probe_pod_prints_its_marker_and_termination_message: SERVICE_K8S_LIVE_KUBE=1 is not set"
        );
        return;
    }
    if !on_kind_cluster() {
        eprintln!(
            "skip probe_pod_prints_its_marker_and_termination_message: the current context is not a kind cluster"
        );
        return;
    }

    let kube = e2e_support::Kube {
        namespace: "default".to_string(),
    };
    let name = format!("probe-kind-{}", std::process::id());
    let script = probe_script("127.0.0.1");
    let overrides = json!({"spec": {"containers": [probe_container(&name, &script)]}});

    kube.run(&[
        "run",
        &name,
        "--restart=Never",
        "--image",
        PROBE_IMAGE,
        "--overrides",
        &overrides.to_string(),
    ]);
    let guard = PodGuard {
        kube: &kube,
        name: name.clone(),
    };
    kube.run(&[
        "wait",
        "--for=jsonpath={.status.phase}=Succeeded",
        &format!("pod/{name}"),
        "--timeout=180s",
    ]);
    let logs = kube.run(&["logs", &name]);
    let pod = kube.get(&format!("pod/{name}"));
    let message = pod["status"]["containerStatuses"][0]["state"]["terminated"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    drop(guard);

    assert!(
        logs.contains("wget-exit="),
        "the log has no marker line: {logs:?}"
    );
    assert!(
        message.contains("wget-exit="),
        "the termination message has no marker line: {message:?}"
    );
    let text = probe_text("", &message).expect("the termination message is used");
    assert_eq!(probe_outcome(&text), Ok(Probe::Reachable));
}
