//! Leader-loss drill on the shared GKE test cluster (core#380).
//!
//! The drill deletes one pod of the rendered `drill` StatefulSet. The pods run
//! a no-op image, so there is no raft leader: the pod stands in for a lost
//! leader. The drill checks rescheduling, placement and status conditions. It
//! does not check acknowledged writes (core#361).

use std::time::Duration;

use serde_json::{json, Value};

use crate::gke_drill::{
    on_gke_drill_cluster, ready_pod, DRILL_POD_LABEL, POOL_KEY, POOL_SERVER, PULL_SECRET,
};

fn pod(name: &str, uid: &str, node: &str, ready: &str) -> Value {
    json!({
        "metadata": { "name": name, "uid": uid },
        "spec": { "nodeName": node },
        "status": {
            "phase": "Running",
            "conditions": [{ "type": "Ready", "status": ready }]
        }
    })
}

fn three_pods(drill0_uid: &str, drill0_ready: &str) -> Value {
    json!({
        "items": [
            pod("drill-0", drill0_uid, "n1", drill0_ready),
            pod("drill-1", "u1", "n2", "True"),
            pod("drill-2", "u2", "n3", "True"),
        ]
    })
}

/// The pod that the drill deletes: ordinal 0 of StatefulSet `drill`.
const LOST_POD: &str = "drill-0";

/// The `metadata.uid` of the pod `name`, or `None` when it is absent.
fn pod_uid(pods: &Value, name: &str) -> Option<String> {
    pod_field(pods, name, "/metadata/uid")
}

/// The `spec.nodeName` of the pod `name`, or `None` when it is absent.
fn pod_node(pods: &Value, name: &str) -> Option<String> {
    pod_field(pods, name, "/spec/nodeName")
}

fn pod_field(pods: &Value, name: &str, pointer: &str) -> Option<String> {
    pods["items"]
        .as_array()?
        .iter()
        .find(|p| p["metadata"]["name"] == name)?
        .pointer(pointer)?
        .as_str()
        .map(str::to_string)
}

/// `None` when a new, Running and Ready pod `name` replaced the deleted one
/// and the pod count is `want_pods`. Otherwise the first failing reason.
fn rescheduled_error(
    before_uid: &str,
    pods: &Value,
    name: &str,
    want_pods: usize,
) -> Option<String> {
    let count = pods["items"].as_array().map_or(0, Vec::len);
    if count != want_pods {
        return Some(format!("expected {want_pods} pods, found {count}"));
    }
    let Some(uid) = pod_uid(pods, name) else {
        return Some(format!("pod {name} is missing"));
    };
    if uid == before_uid {
        return Some(format!(
            "pod {name} still has the old uid {uid}; it was not rescheduled"
        ));
    }
    let single = json!({
        "items": pods["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["metadata"]["name"] == name)
            .cloned()
            .collect::<Vec<Value>>()
    });
    match ready_pod(&single) {
        Ok(_) => None,
        Err(reason) => Some(format!("pod {name} is not Running and Ready: {reason}")),
    }
}

/// `None` when the StatefulSet reports `want` ready and `want` total replicas.
fn statefulset_ready_error(sts: &Value, want: u64) -> Option<String> {
    let ready = sts["status"]["readyReplicas"].as_u64().unwrap_or(0);
    let total = sts["status"]["replicas"].as_u64().unwrap_or(0);
    if ready == want && total == want {
        None
    } else {
        Some(format!(
            "statefulset drill has readyReplicas {ready} and replicas {total}, want {want}"
        ))
    }
}

/// `None` when every pod has a node and no two pods share one.
fn spread_error(pods: &Value) -> Option<String> {
    let mut nodes: Vec<String> = Vec::new();
    for p in pods["items"].as_array().into_iter().flatten() {
        let name = p["metadata"]["name"].as_str().unwrap_or("?");
        let Some(node) = p["spec"]["nodeName"].as_str().filter(|n| !n.is_empty()) else {
            return Some(format!("pod {name} has no node"));
        };
        if nodes.iter().any(|n| n == node) {
            return Some(format!("two drill pods share node {node}"));
        }
        nodes.push(node.to_string());
    }
    None
}

#[test]
fn rescheduled_ok_for_a_new_ready_pod() {
    let pods = three_pods("new", "True");
    assert_eq!(rescheduled_error("old", &pods, "drill-0", 3), None);
}

#[test]
fn rescheduled_error_when_the_uid_did_not_change() {
    // Negative control: the same uid means the old pod was not deleted.
    let pods = three_pods("old", "True");
    let err = rescheduled_error("old", &pods, "drill-0", 3).expect("an unchanged uid is an error");
    assert!(err.contains("uid"), "reason names the uid: {err}");
}

#[test]
fn rescheduled_error_when_the_pod_is_not_ready() {
    let pods = three_pods("new", "False");
    assert!(rescheduled_error("old", &pods, "drill-0", 3).is_some());
}

#[test]
fn rescheduled_error_when_a_pod_is_missing() {
    let pods = json!({
        "items": [
            pod("drill-1", "u1", "n2", "True"),
            pod("drill-2", "u2", "n3", "True"),
        ]
    });
    assert!(rescheduled_error("old", &pods, "drill-0", 3).is_some());
}

#[test]
fn statefulset_ready_error_checks_both_counts() {
    let ready = json!({"status": {"replicas": 3, "readyReplicas": 3}});
    assert_eq!(statefulset_ready_error(&ready, 3), None);

    let short = json!({"status": {"replicas": 3, "readyReplicas": 2}});
    let err = statefulset_ready_error(&short, 3).expect("two of three ready is an error");
    assert!(err.contains('2'), "reason shows the ready count: {err}");
}

#[test]
fn spread_error_finds_two_pods_on_one_node() {
    let shared = json!({
        "items": [
            pod("drill-0", "a", "n1", "True"),
            pod("drill-1", "b", "n1", "True"),
            pod("drill-2", "c", "n2", "True"),
        ]
    });
    let err = spread_error(&shared).expect("two pods on n1 is an error");
    assert!(err.contains("n1"), "reason names the node: {err}");

    let spread = json!({
        "items": [
            pod("drill-0", "a", "n1", "True"),
            pod("drill-1", "b", "n2", "True"),
            pod("drill-2", "c", "n3", "True"),
        ]
    });
    assert_eq!(spread_error(&spread), None);
}

#[test]
#[ignore = "needs the shared GKE cluster: run by .github/workflows/gke-drill.yml (core#380), which sets GKE_DRILL=1, KUBECONFIG and NAMESPACE"]
fn leader_loss_pod_is_rescheduled_on_the_core_server_pool() {
    if !on_gke_drill_cluster("leader_loss_pod_is_rescheduled_on_the_core_server_pool") {
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
    let step = e2e_support::Step::begin("gke-drill", "leader-loss");

    e2e_support::wait_ready(&kube, DRILL_POD_LABEL, 3, Duration::from_secs(600));
    let before = kube.pods(DRILL_POD_LABEL);
    let before_uid = pod_uid(&before, LOST_POD).expect("drill-0 exists before the drill");
    let before_node = pod_node(&before, LOST_POD).unwrap_or_default();
    ready_pod(&before).expect("a drill pod is ready before the drill");

    kube.run(&["delete", "pod", LOST_POD, "--wait=true"]);

    let after = e2e_support::wait_for(
        "drill-0 rescheduled and ready",
        Duration::from_secs(600),
        || {
            let text = kube.try_run(&["get", "pods", "-l", DRILL_POD_LABEL, "-o", "json"])?;
            let pods: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            rescheduled_error(&before_uid, &pods, LOST_POD, 3).map_or(Ok(pods), Err)
        },
    );

    let placement = e2e_support::PoolPlacement {
        key: POOL_KEY.into(),
        value: POOL_SERVER.into(),
        pull_secret: PULL_SECRET.into(),
    };
    for pod in after["items"].as_array().cloned().unwrap_or_default() {
        placement.assert_on_pool(&pod);
    }
    assert_eq!(
        spread_error(&after),
        None,
        "drill pods are not one per node"
    );

    e2e_support::wait_for("statefulset drill ready", Duration::from_secs(120), || {
        let text = kube.try_run(&["get", "statefulset/drill", "-o", "json"])?;
        let sts: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        statefulset_ready_error(&sts, 3).map_or(Ok(()), Err)
    });

    let node_after = pod_node(&after, LOST_POD).unwrap_or_default();
    e2e_support::say(format!(
        "{LOST_POD} on {before_node} before, {node_after} after"
    ));
    e2e_support::evidence(
        "gke-drill",
        "leader-loss",
        json!({
            "pod": LOST_POD,
            "node_before": before_node,
            "node_after": node_after,
            "uid_changed": true
        }),
    );
    step.end();
}
