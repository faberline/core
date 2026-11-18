//! Upgrade-rollback drill on the shared GKE test cluster (core#374).
//!
//! The drill applies the old render, then the current render, then the old
//! render again, with server-side apply. After each apply every object must
//! keep its uid and its `spec`. It checks no `status` and no write: the status
//! proof is the in-process test of #373, and acknowledged writes are in
//! core#361. The drill creates no CRD and runs no old operator binary.

#![cfg(feature = "controller")]

use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};

use crate::gke_drill::on_gke_drill_cluster;
use crate::support::upgrade_rollback::{current_render, load_old_render, old_render_from_env};

const FIELD_MANAGER: &str = "gke-drill-upgrade-rollback";

/// Returns a copy of `objects` in which each object's `metadata.namespace` is `namespace`.
pub fn retarget(objects: &[Value], namespace: &str) -> Vec<Value> {
    objects
        .iter()
        .map(|object| {
            let mut copy = object.clone();
            copy["metadata"]["namespace"] = json!(namespace);
            copy
        })
        .collect()
}

/// `None` when the uid and the `spec` are unchanged. Otherwise the reason.
pub fn survival_error(before: &Value, after: &Value) -> Option<String> {
    let before_uid = before["metadata"]["uid"].as_str().filter(|s| !s.is_empty());
    let after_uid = after["metadata"]["uid"].as_str().filter(|s| !s.is_empty());
    match (before_uid, after_uid) {
        (Some(b), Some(a)) if b == a => {}
        (b, a) => {
            return Some(format!(
                "uid changed or is missing: before uid {b:?}, after uid {a:?}"
            ))
        }
    }
    if before["spec"] != after["spec"] {
        return Some(format!(
            "spec changed: before {}, after {}",
            before["spec"], after["spec"]
        ));
    }
    None
}

/// `<kind lowercase>/<metadata.name>`, for example `service/probe-client`.
pub fn object_ref(object: &Value) -> String {
    match (object["kind"].as_str(), object["metadata"]["name"].as_str()) {
        (Some(kind), Some(name)) => format!("{}/{name}", kind.to_lowercase()),
        _ => panic!("object without kind or name"),
    }
}

/// Applies `objects` as one YAML file with server-side apply.
fn apply(kube: &e2e_support::Kube, label: &str, objects: &[Value]) {
    let yaml = objects
        .iter()
        .map(|object| serde_yaml::to_string(object).expect("object serializes as YAML"))
        .collect::<Vec<_>>()
        .join("---\n");
    let path = std::env::temp_dir().join(format!("gke-drill-upgrade-rollback-{label}.yaml"));
    std::fs::write(&path, yaml).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    let path = path.to_str().expect("temp path is UTF-8");
    kube.run(&[
        "apply",
        "--server-side",
        "--force-conflicts",
        "--field-manager",
        FIELD_MANAGER,
        "-f",
        path,
    ]);
}

/// The old render: from the file named by the environment, or built from the newest `v0.*` tag.
fn old_render() -> Vec<Value> {
    if let Some(objects) = old_render_from_env() {
        return objects;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fetch = Command::new("git")
        .current_dir(&root)
        .args([
            "fetch",
            "--no-tags",
            "--depth=1",
            "origin",
            "refs/tags/v0.*:refs/tags/v0.*",
        ])
        .output()
        .expect("run git fetch");
    assert!(
        fetch.status.success(),
        "git fetch of the v0 tags failed: {}",
        String::from_utf8_lossy(&fetch.stderr)
    );
    let out = std::env::temp_dir().join("gke-drill-upgrade-rollback-old.json");
    let script = Command::new("bash")
        .current_dir(&root)
        .arg("ci/upgrade-rollback-old-render.sh")
        .arg(&out)
        .output()
        .expect("run ci/upgrade-rollback-old-render.sh");
    assert!(
        script.status.success(),
        "ci/upgrade-rollback-old-render.sh failed: {}",
        String::from_utf8_lossy(&script.stderr)
    );
    load_old_render(&out)
}

/// Deletes the drill objects on every exit path, so a failed run leaves none behind.
struct Cleanup<'a> {
    kube: &'a e2e_support::Kube,
    refs: Vec<String>,
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        for object in &self.refs {
            let _ = self.kube.try_run(&["delete", object, "--ignore-not-found"]);
        }
    }
}

#[test]
#[ignore = "needs the shared GKE cluster: run by .github/workflows/gke-drill.yml (core#374), which sets GKE_DRILL=1, KUBECONFIG and NAMESPACE"]
fn upgrade_and_rollback_keep_the_objects_on_a_real_apiserver() {
    if !on_gke_drill_cluster("upgrade_and_rollback_keep_the_objects_on_a_real_apiserver") {
        return;
    }
    let kube = e2e_support::Kube::from_env();
    let _dump = e2e_support::DumpOnFailure {
        suite: "gke-drill",
        kube: &kube,
        dump_commands: vec![vec![
            "get".into(),
            "serviceaccount,service".into(),
            "-o".into(),
            "yaml".into(),
        ]],
    };
    let step = e2e_support::Step::begin("gke-drill", "upgrade-rollback");

    let namespace = e2e_support::namespace();
    let old = retarget(&old_render(), &namespace);
    let current = retarget(&current_render(), &namespace);
    let refs: Vec<String> = current.iter().map(object_ref).collect();
    assert_eq!(
        refs,
        ["serviceaccount/probe", "service/probe-client"],
        "the current render names the drill objects"
    );
    let old_refs: Vec<String> = old.iter().map(object_ref).collect();
    assert_eq!(
        old_refs, refs,
        "the old render names different objects than the current render"
    );
    let _cleanup = Cleanup {
        kube: &kube,
        refs: refs.clone(),
    };

    apply(&kube, "old", &old);
    let before: Vec<Value> = refs.iter().map(|r| kube.get(r)).collect();

    apply(&kube, "current", &current);
    let upgraded: Vec<Value> = refs.iter().map(|r| kube.get(r)).collect();
    for (i, r) in refs.iter().enumerate() {
        assert_eq!(
            survival_error(&before[i], &upgraded[i]),
            None,
            "{r} after the upgrade"
        );
    }

    apply(&kube, "rollback", &old);
    let rolled_back: Vec<Value> = refs.iter().map(|r| kube.get(r)).collect();
    for (i, r) in refs.iter().enumerate() {
        assert_eq!(
            survival_error(&before[i], &rolled_back[i]),
            None,
            "{r} after the rollback"
        );
    }

    let uids: Vec<Value> = before
        .iter()
        .map(|object| object["metadata"]["uid"].clone())
        .collect();
    e2e_support::evidence(
        "gke-drill",
        "upgrade-rollback",
        json!({ "refs": refs, "uids": uids }),
    );
    step.end();
}

#[test]
fn retarget_sets_the_namespace_of_every_object() {
    let input = vec![
        json!({"kind": "ServiceAccount", "metadata": {"name": "a", "namespace": "x"}}),
        json!({"kind": "Service", "metadata": {"name": "b"}}),
    ];
    let out = retarget(&input, "core");
    assert_eq!(out.len(), 2);
    assert_eq!(out[0]["metadata"]["namespace"], "core");
    assert_eq!(out[1]["metadata"]["namespace"], "core");
    assert_eq!(
        input[0]["metadata"]["namespace"], "x",
        "input is not changed"
    );
    assert!(input[1]["metadata"].get("namespace").is_none());
}

#[test]
fn object_ref_names_kind_and_name() {
    let service = json!({"kind": "Service", "metadata": {"name": "probe-client"}});
    assert_eq!(object_ref(&service), "service/probe-client");
}

#[test]
#[should_panic(expected = "object without kind or name")]
fn object_ref_panics_without_a_name() {
    object_ref(&json!({"kind": "Service", "metadata": {}}));
}

#[test]
fn survival_ok_for_the_same_uid_and_spec() {
    let object = json!({"metadata": {"uid": "u1"}, "spec": {"ports": [{"port": 8080}]}});
    assert_eq!(survival_error(&object, &object.clone()), None);
}

#[test]
fn survival_error_when_the_uid_changes() {
    // Negative control: a new uid must fail.
    let before = json!({"metadata": {"uid": "u1"}, "spec": {}});
    let after = json!({"metadata": {"uid": "u2"}, "spec": {}});
    let err = survival_error(&before, &after).expect("a changed uid is an error");
    assert!(err.contains("uid"), "reason names the uid: {err}");
    assert!(err.contains("u1"), "reason shows the old uid: {err}");
    assert!(err.contains("u2"), "reason shows the new uid: {err}");
}

#[test]
fn survival_error_when_the_spec_changes() {
    // Negative control: the same uid with a changed port must fail.
    let before = json!({"metadata": {"uid": "u1"}, "spec": {"ports": [{"port": 8080}]}});
    let after = json!({"metadata": {"uid": "u1"}, "spec": {"ports": [{"port": 9090}]}});
    let err = survival_error(&before, &after).expect("a changed spec is an error");
    assert!(err.contains("spec"), "reason names the spec: {err}");
}

#[test]
fn survival_error_when_the_uid_is_missing() {
    let before = json!({"metadata": {}, "spec": {}});
    let after = json!({"metadata": {"uid": "u1"}, "spec": {}});
    let err = survival_error(&before, &after).expect("a missing uid is an error");
    assert!(err.contains("uid"), "reason names the uid: {err}");
}

#[test]
fn the_two_renders_name_the_same_objects() {
    let refs: Vec<String> = current_render().iter().map(object_ref).collect();
    assert_eq!(refs, ["serviceaccount/probe", "service/probe-client"]);
}
