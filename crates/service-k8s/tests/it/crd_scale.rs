//! The CRD `subresources.scale` block: learners services expose it, the others do not.

use serde_json::json;
use service_k8s::crd::{add_scale_subresource, scale_subresource};

fn crd() -> serde_json::Value {
    json!({ "spec": { "versions": [{ "name": "v1" }] } })
}

#[test]
fn learners_crd_has_scale_spec_replicas_path() {
    let mut crd = crd();

    assert_eq!(add_scale_subresource(&mut crd, true), 1);
    assert_eq!(
        crd["spec"]["versions"][0]["subresources"]["scale"]["specReplicasPath"],
        ".spec.replicasPerShard"
    );
}

#[test]
fn none_crd_has_no_scale_key() {
    let mut crd = crd();

    assert_eq!(add_scale_subresource(&mut crd, false), 0);
    assert!(crd["spec"]["versions"][0].get("subresources").is_none());
}

#[test]
fn scale_subresource_none_for_false() {
    assert!(scale_subresource(false).is_none());
}

#[test]
fn static_membership_doc_is_gone() {
    let capacity = include_str!("../../src/domain/capacity.rs");
    let cluster = include_str!("../../src/interfaces/cluster_spec.rs");

    assert!(!capacity.contains("currently has static membership"));
    assert!(!cluster.contains("startup-static"));
}
