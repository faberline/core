//! Rolling-upgrade drill on the shared GKE test cluster (core#382).
//!
//! The drill patches the pod template of the rendered `drill` StatefulSet. It
//! checks the rollout order (highest ordinal first) and the status conditions.
//! The pods run `pause`, so the drill does not check acknowledged writes
//! (core#361).

use std::time::Duration;

use serde_json::{json, Value};

use crate::gke_drill::{on_gke_drill_cluster, ready_pod, DRILL_POD_LABEL, DRILL_REPLICAS};

const REV_ANNOTATION: &str = "core.faberline.dev/drill-rev";
const MIN_READY_SECONDS: u64 = 5;

fn pod(name: &str, uid: &str, created: &str) -> Value {
    json!({
        "metadata": { "name": name, "uid": uid, "creationTimestamp": created }
    })
}

fn pods(items: Vec<Value>) -> Value {
    json!({ "items": items })
}

fn before_rollout() -> Value {
    pods(vec![
        pod("drill-0", "a0", "2026-11-15T01:00:00Z"),
        pod("drill-1", "a1", "2026-11-15T01:00:00Z"),
        pod("drill-2", "a2", "2026-11-15T01:00:00Z"),
    ])
}

/// The patch that changes the pod template and sets `minReadySeconds`.
pub fn rollout_patch(rev: &str) -> Value {
    json!({
        "spec": {
            "minReadySeconds": MIN_READY_SECONDS,
            "template": { "metadata": { "annotations": { REV_ANNOTATION: rev } } }
        }
    })
}

/// The string at JSON pointer `pointer` of the pod `name`, or `None`.
fn pod_field(pods: &Value, name: &str, pointer: &str) -> Option<String> {
    pods["items"]
        .as_array()?
        .iter()
        .find(|p| p["metadata"]["name"] == name)?
        .pointer(pointer)?
        .as_str()
        .map(str::to_string)
}

/// `None` when the rollout replaced all three pods from the highest ordinal
/// down. Otherwise the first failing reason.
pub fn rollout_order_error(before: &Value, after: &Value) -> Option<String> {
    let want = DRILL_REPLICAS as usize;
    let count = after["items"].as_array().map_or(0, Vec::len);
    if count != want {
        return Some(format!(
            "expected {want} pods after the rollout, found {count}"
        ));
    }
    let names = ["drill-0", "drill-1", "drill-2"];
    for name in names {
        let Some(new_uid) = pod_field(after, name, "/metadata/uid") else {
            return Some(format!("pod {name} is missing after the rollout"));
        };
        if pod_field(before, name, "/metadata/uid").as_deref() == Some(new_uid.as_str()) {
            return Some(format!(
                "pod {name} was not replaced: uid {new_uid} is not new"
            ));
        }
    }
    let created = |name: &str| pod_field(after, name, "/metadata/creationTimestamp");
    let (c2, c1, c0) = (created("drill-2"), created("drill-1"), created("drill-0"));
    let in_order = match (&c2, &c1, &c0) {
        (Some(a), Some(b), Some(c)) => a < b && b < c,
        _ => false,
    };
    if !in_order {
        return Some(format!(
            "rollout order is wrong: drill-2 at {c2:?}, drill-1 at {c1:?}, drill-0 at {c0:?}; \
             want drill-2 < drill-1 < drill-0"
        ));
    }
    None
}

/// `None` when the StatefulSet reports `want` replicas that are updated and
/// ready, on one revision, for the current generation.
pub fn statefulset_rolled_error(sts: &Value, want: u64) -> Option<String> {
    let status = &sts["status"];
    let replicas = status["replicas"].as_u64().unwrap_or(0);
    let updated = status["updatedReplicas"].as_u64().unwrap_or(0);
    let ready = status["readyReplicas"].as_u64().unwrap_or(0);
    if replicas != want || updated != want || ready != want {
        return Some(format!(
            "statefulset drill has replicas {replicas}, updatedReplicas {updated}, \
             readyReplicas {ready}, want {want}"
        ));
    }
    let current = status["currentRevision"].as_str().unwrap_or("");
    let update = status["updateRevision"].as_str().unwrap_or("");
    if current.is_empty() || current != update {
        return Some(format!(
            "statefulset drill has currentRevision {current} and updateRevision {update}, \
             they must be equal"
        ));
    }
    let generation = sts["metadata"]["generation"].as_u64();
    let observed = status["observedGeneration"].as_u64();
    if generation.is_none() || observed != generation {
        return Some(format!(
            "statefulset drill has observedGeneration {observed:?}, want generation {generation:?}"
        ));
    }
    None
}

#[test]
fn rollout_patch_changes_the_template_and_min_ready() {
    let patch = rollout_patch("2");
    assert_eq!(
        patch["spec"]["template"]["metadata"]["annotations"]["core.faberline.dev/drill-rev"],
        "2"
    );
    assert_eq!(patch["spec"]["minReadySeconds"], 5);
}

#[test]
fn rollout_order_ok_from_the_highest_ordinal() {
    let after = pods(vec![
        pod("drill-0", "b0", "2026-11-15T01:01:20Z"),
        pod("drill-1", "b1", "2026-11-15T01:01:10Z"),
        pod("drill-2", "b2", "2026-11-15T01:01:00Z"),
    ]);
    assert_eq!(rollout_order_error(&before_rollout(), &after), None);
}

#[test]
fn rollout_order_error_when_drill_0_goes_first() {
    // Negative control: the order check must fail when drill-0 is replaced first.
    let after = pods(vec![
        pod("drill-0", "b0", "2026-11-15T01:01:00Z"),
        pod("drill-1", "b1", "2026-11-15T01:01:10Z"),
        pod("drill-2", "b2", "2026-11-15T01:01:20Z"),
    ]);
    let err =
        rollout_order_error(&before_rollout(), &after).expect("drill-0 first is an order error");
    assert!(err.contains("order"), "reason names the order: {err}");
}

#[test]
fn rollout_order_error_when_a_pod_was_not_replaced() {
    // Negative control: drill-1 keeps its old uid.
    let after = pods(vec![
        pod("drill-0", "b0", "2026-11-15T01:01:20Z"),
        pod("drill-1", "a1", "2026-11-15T01:00:00Z"),
        pod("drill-2", "b2", "2026-11-15T01:01:00Z"),
    ]);
    let err = rollout_order_error(&before_rollout(), &after).expect("an old uid is an error");
    assert!(err.contains("drill-1"), "reason names the pod: {err}");
    assert!(
        err.contains("not replaced"),
        "reason says not replaced: {err}"
    );
}

#[test]
fn rollout_order_error_when_a_pod_is_missing() {
    let after = pods(vec![
        pod("drill-1", "b1", "2026-11-15T01:01:10Z"),
        pod("drill-2", "b2", "2026-11-15T01:01:00Z"),
    ]);
    assert!(rollout_order_error(&before_rollout(), &after).is_some());
}

fn rolled_statefulset(current: &str, ready: u64) -> Value {
    json!({
        "metadata": { "generation": 2 },
        "status": {
            "replicas": 3,
            "updatedReplicas": 3,
            "readyReplicas": ready,
            "currentRevision": current,
            "updateRevision": "drill-7",
            "observedGeneration": 2
        }
    })
}

#[test]
fn statefulset_rolled_ok() {
    assert_eq!(
        statefulset_rolled_error(&rolled_statefulset("drill-7", 3), 3),
        None
    );
}

#[test]
fn statefulset_rolled_error_while_revisions_differ() {
    let sts = rolled_statefulset("drill-6", 3);
    let err = statefulset_rolled_error(&sts, 3).expect("differing revisions are an error");
    assert!(err.contains("drill-6"), "reason shows the revision: {err}");
}

#[test]
fn statefulset_rolled_error_when_a_replica_is_not_ready() {
    let sts = rolled_statefulset("drill-7", 2);
    let err = statefulset_rolled_error(&sts, 3).expect("two of three ready is an error");
    assert!(err.contains('2'), "reason shows the ready count: {err}");
}

#[test]
#[ignore = "needs the shared GKE cluster: run by .github/workflows/gke-drill.yml (core#382), which sets GKE_DRILL=1, KUBECONFIG and NAMESPACE"]
fn rolling_upgrade_replaces_the_pods_from_the_highest_ordinal() {
    if !on_gke_drill_cluster("rolling_upgrade_replaces_the_pods_from_the_highest_ordinal") {
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
    let step = e2e_support::Step::begin("gke-drill", "rolling-upgrade");

    e2e_support::wait_ready(&kube, DRILL_POD_LABEL, 3, Duration::from_secs(600));
    let before = kube.pods(DRILL_POD_LABEL);

    kube.run(&[
        "patch",
        "statefulset",
        "drill",
        "--type",
        "merge",
        "-p",
        &rollout_patch("2").to_string(),
    ]);
    kube.run(&["rollout", "status", "statefulset/drill", "--timeout=600s"]);
    e2e_support::wait_ready(&kube, DRILL_POD_LABEL, 3, Duration::from_secs(300));
    let after = kube.pods(DRILL_POD_LABEL);

    assert_eq!(
        rollout_order_error(&before, &after),
        None,
        "rollout order is wrong"
    );
    assert!(ready_pod(&after).is_ok(), "no ready pod after the rollout");
    e2e_support::wait_for("statefulset drill rolled", Duration::from_secs(120), || {
        let text = kube.try_run(&["get", "statefulset/drill", "-o", "json"])?;
        let sts: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        statefulset_rolled_error(&sts, 3).map_or(Ok(()), Err)
    });

    let created = |name: &str| pod_field(&after, name, "/metadata/creationTimestamp");
    e2e_support::evidence(
        "gke-drill",
        "rolling-upgrade",
        json!({
            "order": ["drill-2", "drill-1", "drill-0"],
            "created": [created("drill-2"), created("drill-1"), created("drill-0")],
        }),
    );
    step.end();
}
