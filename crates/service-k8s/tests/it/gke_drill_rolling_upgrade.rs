//! Rolling-upgrade drill on the shared GKE test cluster (core#382).
//!
//! The drill changes the pod template of the rendered `drill` StatefulSet and
//! checks the rollout order and the status conditions. The pods run `pause`.
//! The drill checks no write (core#361).

use std::time::Duration;

use serde_json::{json, Value};

use crate::gke_drill::{on_gke_drill_cluster, ready_pod, DRILL_POD_LABEL, DRILL_REPLICAS};

fn pod(name: &str, uid: &str, created: &str) -> Value {
    json!({"metadata": {"name": name, "uid": uid, "creationTimestamp": created}})
}

fn pods(items: Vec<Value>) -> Value {
    json!({ "items": items })
}

/// A pod annotation that changes the pod template, so every pod is rolled.
const REV_ANNOTATION: &str = "core.faberline.dev/drill-rev";
/// Each new pod stays not-yet-available for this long, so that two pod
/// creation times (one-second resolution) differ.
const MIN_READY_SECONDS: u64 = 5;

/// The merge patch that changes the template and sets `minReadySeconds`.
fn rollout_patch(rev: &str) -> Value {
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

/// `None` when all of `drill-0..2` were replaced, in the order `drill-2`,
/// `drill-1`, `drill-0` by creation time. Otherwise the first failing reason.
fn rollout_order_error(before: &Value, after: &Value) -> Option<String> {
    let want = DRILL_REPLICAS as usize;
    let count = after["items"].as_array().map_or(0, Vec::len);
    if count != want {
        return Some(format!("expected {want} pods, found {count}"));
    }
    let names = ["drill-0", "drill-1", "drill-2"];
    for name in names {
        let Some(after_uid) = pod_field(after, name, "/metadata/uid") else {
            return Some(format!("pod {name} is missing"));
        };
        if pod_field(before, name, "/metadata/uid").as_deref() == Some(after_uid.as_str()) {
            return Some(format!(
                "pod {name} was not replaced (same uid {after_uid})"
            ));
        }
    }
    let created = |name: &str| pod_field(after, name, "/metadata/creationTimestamp");
    let (Some(c2), Some(c1), Some(c0)) =
        (created("drill-2"), created("drill-1"), created("drill-0"))
    else {
        return Some("a pod has no creationTimestamp".to_string());
    };
    if c2 < c1 && c1 < c0 {
        None
    } else {
        Some(format!(
            "rollout order is wrong: drill-2 {c2}, drill-1 {c1}, drill-0 {c0}"
        ))
    }
}

/// `None` when the StatefulSet has `want` updated and ready replicas, one
/// revision, and has observed its newest generation.
fn statefulset_rolled_error(sts: &Value, want: u64) -> Option<String> {
    let status = &sts["status"];
    let replicas = status["replicas"].as_u64().unwrap_or(0);
    let updated = status["updatedReplicas"].as_u64().unwrap_or(0);
    let ready = status["readyReplicas"].as_u64().unwrap_or(0);
    if replicas != want || updated != want || ready != want {
        return Some(format!(
            "statefulset drill has replicas {replicas}, updatedReplicas {updated}, readyReplicas {ready}, want {want}"
        ));
    }
    let current = status["currentRevision"].as_str().unwrap_or("");
    let update = status["updateRevision"].as_str().unwrap_or("");
    if current.is_empty() || current != update {
        return Some(format!(
            "statefulset drill revisions differ: current {current}, update {update}"
        ));
    }
    if status["observedGeneration"] != sts["metadata"]["generation"] {
        return Some("statefulset drill has not observed its newest generation".to_string());
    }
    None
}

fn before_pods() -> Value {
    pods(vec![
        pod("drill-0", "a0", "2026-11-15T01:00:00Z"),
        pod("drill-1", "a1", "2026-11-15T01:00:00Z"),
        pod("drill-2", "a2", "2026-11-15T01:00:00Z"),
    ])
}

#[test]
fn rollout_order_ok_from_the_highest_ordinal() {
    let after = pods(vec![
        pod("drill-0", "b0", "2026-11-15T01:01:20Z"),
        pod("drill-1", "b1", "2026-11-15T01:01:10Z"),
        pod("drill-2", "b2", "2026-11-15T01:01:00Z"),
    ]);
    assert_eq!(rollout_order_error(&before_pods(), &after), None);
}

#[test]
fn rollout_order_error_when_drill_0_goes_first() {
    // Negative control: the order check must reject this.
    let after = pods(vec![
        pod("drill-0", "b0", "2026-11-15T01:01:00Z"),
        pod("drill-1", "b1", "2026-11-15T01:01:10Z"),
        pod("drill-2", "b2", "2026-11-15T01:01:20Z"),
    ]);
    let err = rollout_order_error(&before_pods(), &after).expect("drill-0 first is an error");
    assert!(err.contains("order"), "{err}");
}

#[test]
fn rollout_order_error_when_a_pod_was_not_replaced() {
    let after = pods(vec![
        pod("drill-0", "b0", "2026-11-15T01:01:20Z"),
        pod("drill-1", "a1", "2026-11-15T01:01:10Z"),
        pod("drill-2", "b2", "2026-11-15T01:01:00Z"),
    ]);
    let err = rollout_order_error(&before_pods(), &after).expect("an old uid is an error");
    assert!(
        err.contains("drill-1") && err.contains("not replaced"),
        "{err}"
    );
}

#[test]
fn rollout_order_error_when_a_pod_is_missing() {
    let after = pods(vec![
        pod("drill-1", "b1", "2026-11-15T01:01:10Z"),
        pod("drill-2", "b2", "2026-11-15T01:01:00Z"),
    ]);
    assert!(rollout_order_error(&before_pods(), &after).is_some());
}

fn rolled_sts(ready: u64, revision: &str) -> Value {
    json!({
        "metadata": { "generation": 2 },
        "status": {
            "replicas": 3,
            "updatedReplicas": 3,
            "readyReplicas": ready,
            "currentRevision": revision,
            "updateRevision": "drill-7",
            "observedGeneration": 2
        }
    })
}

#[test]
fn statefulset_rolled_ok() {
    assert_eq!(statefulset_rolled_error(&rolled_sts(3, "drill-7"), 3), None);
}

#[test]
fn statefulset_rolled_error_while_revisions_differ() {
    let err = statefulset_rolled_error(&rolled_sts(3, "drill-6"), 3)
        .expect("revisions that differ are an error");
    assert!(err.contains("drill-6"), "{err}");
}

#[test]
fn statefulset_rolled_error_when_a_replica_is_not_ready() {
    let err = statefulset_rolled_error(&rolled_sts(2, "drill-7"), 3)
        .expect("two of three ready is an error");
    assert!(err.contains('2'), "{err}");
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
            "created": [created("drill-2"), created("drill-1"), created("drill-0")]
        }),
    );
    step.end();
}
