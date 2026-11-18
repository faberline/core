//! Rolling update and rollback on the shared GKE test cluster (core#385).
//!
//! The drill patches the pod template of the `drill` StatefulSet, then runs
//! `kubectl rollout undo`. While both rollouts run it samples the pod list every
//! 2 seconds. The claim is "rolling update and rollback keep 2 of 3 pods Ready".
//!
//! This is NOT a zero-downtime proof under client load (core#359): the pods run
//! `pause` and serve no request, and core ships no serving image (faberline/aw#416,
//! desk decision faberline/aw#611). Do not count this drill as that proof.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::gke_drill::{on_gke_drill_cluster, DRILL_POD_LABEL, DRILL_REPLICAS};
use crate::gke_drill_rolling_upgrade::{rollout_patch, statefulset_rolled_error};

const FLOOR: usize = DRILL_REPLICAS as usize - 1;
const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);

/// Pods that are Running with the condition Ready=True.
pub fn ready_count(pods: &Value) -> usize {
    pods["items"].as_array().map_or(0, |items| {
        items
            .iter()
            .filter(|pod| {
                pod["status"]["phase"] == "Running"
                    && pod["status"]["conditions"].as_array().is_some_and(|conds| {
                        conds
                            .iter()
                            .any(|c| c["type"] == "Ready" && c["status"] == "True")
                    })
            })
            .count()
    })
}

/// The smallest value, or `None` for no samples.
pub fn min_ready(samples: &[usize]) -> Option<usize> {
    samples.iter().copied().min()
}

/// `None` when `samples` is not empty and every sample is at least `floor`.
/// Otherwise the first failing reason.
pub fn availability_error(samples: &[usize], floor: usize) -> Option<String> {
    if samples.is_empty() {
        return Some("no samples were taken".to_string());
    }
    samples
        .iter()
        .enumerate()
        .find(|(_, n)| **n < floor)
        .map(|(i, n)| format!("sample {i} had {n} ready pods, want at least {floor}"))
}

/// Samples the Ready count every SAMPLE_INTERVAL until `done(&sts)` is `None`
/// or `timeout` passes. Returns every sample.
fn sample_until(
    kube: &e2e_support::Kube,
    what: &str,
    timeout: Duration,
    done: impl Fn(&Value) -> Option<String>,
) -> Vec<usize> {
    let start = Instant::now();
    let mut samples = Vec::new();
    loop {
        samples.push(ready_count(&kube.pods(DRILL_POD_LABEL)));
        let sts = kube.get("statefulset/drill");
        let Some(err) = done(&sts) else {
            return samples;
        };
        if start.elapsed() >= timeout {
            panic!(
                "timed out after {}s waiting for {what}: {err}",
                timeout.as_secs()
            );
        }
        std::thread::sleep(SAMPLE_INTERVAL);
    }
}

fn pods_with(ready: &[bool]) -> Value {
    let items: Vec<Value> = ready
        .iter()
        .enumerate()
        .map(|(i, is_ready)| {
            json!({
                "metadata": { "name": format!("drill-{i}") },
                "status": {
                    "phase": "Running",
                    "conditions": [{
                        "type": "Ready",
                        "status": if *is_ready { "True" } else { "False" }
                    }]
                }
            })
        })
        .collect();
    json!({ "items": items })
}

#[test]
fn ready_count_counts_only_running_ready_pods() {
    assert_eq!(ready_count(&pods_with(&[true, true, false])), 2);
    // A Pending pod with Ready=True is not ready.
    let pending = json!({
        "items": [{
            "metadata": { "name": "drill-0" },
            "status": {
                "phase": "Pending",
                "conditions": [{ "type": "Ready", "status": "True" }]
            }
        }]
    });
    assert_eq!(ready_count(&pending), 0);
}

#[test]
fn ready_count_is_zero_without_items() {
    assert_eq!(ready_count(&json!({})), 0);
}

#[test]
fn min_ready_is_the_smallest_sample() {
    assert_eq!(min_ready(&[3, 2, 3]), Some(2));
    assert_eq!(min_ready(&[]), None);
}

#[test]
fn availability_ok_when_every_sample_has_two_ready() {
    assert_eq!(availability_error(&[3, 2, 2, 3], 2), None);
}

#[test]
fn availability_error_when_a_sample_drops_below_the_floor() {
    // Negative control: a check that always returns None fails here.
    let err = availability_error(&[3, 2, 1, 3], 2).expect("1 ready is below the floor");
    assert!(err.contains("sample 2"), "reason names the sample: {err}");
    assert!(err.contains("1 ready"), "reason shows the count: {err}");
}

#[test]
fn availability_error_without_samples() {
    // Negative control: no samples must not pass as available.
    let err = availability_error(&[], 2).expect("no samples is an error");
    assert!(err.contains("no samples"), "reason says so: {err}");
}

#[test]
#[ignore = "needs the shared GKE cluster: run by .github/workflows/gke-drill.yml (core#385), which sets GKE_DRILL=1, KUBECONFIG and NAMESPACE"]
fn rolling_update_and_rollback_keep_two_of_three_pods_ready() {
    if !on_gke_drill_cluster("rolling_update_and_rollback_keep_two_of_three_pods_ready") {
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
                "statefulset".into(),
                "drill".into(),
                "-o".into(),
                "yaml".into(),
            ],
        ],
    };
    let step = e2e_support::Step::begin("gke-drill", "zero-downtime");
    let started = Instant::now();

    e2e_support::wait_ready(&kube, DRILL_POD_LABEL, 3, Duration::from_secs(600));
    let before_revision = kube.get("statefulset/drill")["status"]["updateRevision"]
        .as_str()
        .expect("statefulset drill has an updateRevision")
        .to_string();

    kube.run(&[
        "patch",
        "statefulset",
        "drill",
        "--type",
        "merge",
        "-p",
        &rollout_patch("zero-downtime-up").to_string(),
    ]);
    let up = sample_until(&kube, "upgrade rolled", Duration::from_secs(600), |sts| {
        statefulset_rolled_error(sts, 3)
    });
    let upgraded_revision = kube.get("statefulset/drill")["status"]["updateRevision"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert_ne!(
        upgraded_revision, before_revision,
        "the upgrade must create a new revision"
    );

    kube.run(&["rollout", "undo", "statefulset/drill"]);
    let back = sample_until(&kube, "rollback rolled", Duration::from_secs(600), |sts| {
        statefulset_rolled_error(sts, 3)
    });
    let rolled_back_revision = kube.get("statefulset/drill")["status"]["updateRevision"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert_eq!(
        rolled_back_revision, before_revision,
        "the rollback must return to the revision before the upgrade"
    );

    assert_eq!(availability_error(&up, FLOOR), None, "during the upgrade");
    assert_eq!(
        availability_error(&back, FLOOR),
        None,
        "during the rollback"
    );

    e2e_support::evidence(
        "gke-drill",
        "zero-downtime",
        json!({
            "claim": "rolling update and rollback keep 2 of 3 pods Ready",
            "not_a_proof_of": "zero downtime under client load (core#359): the pods run pause and serve no request",
            "floor": FLOOR,
            "upgrade": { "samples": up.len(), "min_ready": min_ready(&up) },
            "rollback": { "samples": back.len(), "min_ready": min_ready(&back) },
            "elapsed_seconds": started.elapsed().as_secs(),
        }),
    );
    step.end();
}
