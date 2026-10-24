use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;

#[test]
fn group_scoped_host_anti_affinity_hard_and_soft() {
    let hard_plan = PlacementPlan::new("app.kubernetes.io/shard", "shard-0")
        .with_host_separation(HostSeparation::Hard);

    let hard_affinity = hard_plan.affinity().expect("render hard affinity");
    assert_eq!(
        hard_affinity,
        json!({
            "podAntiAffinity": {
                "requiredDuringSchedulingIgnoredDuringExecution": [{
                    "labelSelector": {
                        "matchLabels": {
                            "app.kubernetes.io/shard": "shard-0",
                        }
                    },
                    "topologyKey": "kubernetes.io/hostname",
                }]
            }
        })
    );

    let soft_plan = PlacementPlan::new("app.kubernetes.io/shard", "shard-0")
        .with_host_separation(HostSeparation::Soft(50));

    let soft_affinity = soft_plan.affinity().expect("render soft affinity");
    assert_eq!(
        soft_affinity,
        json!({
            "podAntiAffinity": {
                "preferredDuringSchedulingIgnoredDuringExecution": [{
                    "weight": 50,
                    "podAffinityTerm": {
                        "labelSelector": {
                            "matchLabels": {
                                "app.kubernetes.io/shard": "shard-0",
                            }
                        },
                        "topologyKey": "kubernetes.io/hostname",
                    }
                }]
            }
        })
    );
}

#[test]
fn zone_spread_required_and_preferred() {
    let req_plan = PlacementPlan::new("app.kubernetes.io/group", "core-data")
        .with_zone_spread(ZoneSpread::required(1));

    let req_spread = req_plan
        .topology_spread_constraints()
        .expect("render required zone spread");
    assert_eq!(req_spread.len(), 1);
    assert_eq!(
        req_spread[0],
        json!({
            "maxSkew": 1,
            "topologyKey": "topology.kubernetes.io/zone",
            "whenUnsatisfiable": "DoNotSchedule",
            "labelSelector": {
                "matchLabels": {
                    "app.kubernetes.io/group": "core-data",
                }
            }
        })
    );

    let pref_plan = PlacementPlan::new("app.kubernetes.io/group", "core-data")
        .with_zone_spread(ZoneSpread::preferred(2));

    let pref_spread = pref_plan
        .topology_spread_constraints()
        .expect("render preferred zone spread");
    assert_eq!(pref_spread.len(), 1);
    assert_eq!(
        pref_spread[0],
        json!({
            "maxSkew": 2,
            "topologyKey": "topology.kubernetes.io/zone",
            "whenUnsatisfiable": "ScheduleAnyway",
            "labelSelector": {
                "matchLabels": {
                    "app.kubernetes.io/group": "core-data",
                }
            }
        })
    );
}

#[test]
fn deterministic_group_label_byte_identical() {
    let plan1 = PlacementPlan::new("faberline.dev/replica-group", "group-alpha")
        .with_host_separation(HostSeparation::Hard)
        .with_zone_spread(ZoneSpread::required(1))
        .with_node_selector("topology.kubernetes.io/zone", "us-central1-a")
        .with_node_selector("kubernetes.io/os", "linux")
        .with_toleration(Toleration::equal("dedicated", "data", "NoSchedule"));

    let plan2 = PlacementPlan::new("faberline.dev/replica-group", "group-alpha")
        .with_host_separation(HostSeparation::Hard)
        .with_zone_spread(ZoneSpread::required(1))
        .with_node_selector("kubernetes.io/os", "linux")
        .with_node_selector("topology.kubernetes.io/zone", "us-central1-a")
        .with_toleration(Toleration::equal("dedicated", "data", "NoSchedule"));

    let json1 = serde_json::to_string(&plan1.affinity().unwrap()).unwrap();
    let json2 = serde_json::to_string(&plan2.affinity().unwrap()).unwrap();
    assert_eq!(json1, json2);

    let sel1 = serde_json::to_string(&plan1.node_selector().unwrap()).unwrap();
    let sel2 = serde_json::to_string(&plan2.node_selector().unwrap()).unwrap();
    assert_eq!(sel1, sel2);

    let (k, v) = plan1.group_label();
    assert_eq!(k, "faberline.dev/replica-group");
    assert_eq!(v, "group-alpha");
    assert_eq!(plan1.label_pair(), (k.to_string(), v.to_string()));
}

#[test]
fn malformed_input_refused_with_typed_error() {
    // Empty label key
    let err = PlacementPlan::new("", "val").validate().unwrap_err();
    assert_eq!(err, PlacementError::InvalidLabelKey("".to_string()));

    // Invalid label key syntax
    let err = PlacementPlan::new("-invalid-prefix/name", "val")
        .validate()
        .unwrap_err();
    assert!(matches!(err, PlacementError::InvalidLabelKey(_)));

    let err = PlacementPlan::new("prefix/-invalid-name", "val")
        .validate()
        .unwrap_err();
    assert!(matches!(err, PlacementError::InvalidLabelKey(_)));

    // Empty label value
    let err = PlacementPlan::new("key", "").validate().unwrap_err();
    assert_eq!(err, PlacementError::InvalidLabelValue("".to_string()));

    // Invalid label value syntax (starts with hyphen or has invalid chars)
    let err = PlacementPlan::new("key", "-val").validate().unwrap_err();
    assert_eq!(err, PlacementError::InvalidLabelValue("-val".to_string()));

    let err = PlacementPlan::new("key", "val@bad").validate().unwrap_err();
    assert_eq!(
        err,
        PlacementError::InvalidLabelValue("val@bad".to_string())
    );

    // Empty topology key
    let err = PlacementPlan::new("key", "val")
        .with_zone_spread(ZoneSpread::Required {
            max_skew: 1,
            topology_key: "".to_string(),
        })
        .validate()
        .unwrap_err();
    assert_eq!(err, PlacementError::EmptyTopologyKey);

    // maxSkew == 0
    let err = PlacementPlan::new("key", "val")
        .with_zone_spread(ZoneSpread::required(0))
        .validate()
        .unwrap_err();
    assert_eq!(err, PlacementError::InvalidMaxSkew);

    // Toleration with invalid operator
    let err = PlacementPlan::new("key", "val")
        .with_toleration(Toleration::new("k", "InvalidOp", None, "NoSchedule"))
        .validate()
        .unwrap_err();
    assert_eq!(
        err,
        PlacementError::InvalidTolerationOperator("InvalidOp".to_string())
    );

    // Toleration with invalid effect
    let err = PlacementPlan::new("key", "val")
        .with_toleration(Toleration::new("k", "Equal", Some("v".into()), "BadEffect"))
        .validate()
        .unwrap_err();
    assert_eq!(
        err,
        PlacementError::InvalidTolerationEffect("BadEffect".to_string())
    );
}

#[test]
fn unschedulable_input_refused_when_replicas_exceed_hosts() {
    let feasible = PlacementPlan::new("app.kubernetes.io/shard", "shard-0")
        .with_host_separation(HostSeparation::Hard)
        .with_domain_counts(3, 3);
    assert!(feasible.validate().is_ok());

    let unfeasible = PlacementPlan::new("app.kubernetes.io/shard", "shard-0")
        .with_host_separation(HostSeparation::Hard)
        .with_domain_counts(4, 3);
    let err = unfeasible.validate().unwrap_err();
    assert_eq!(
        err,
        PlacementError::Unschedulable {
            replicas: 4,
            hosts: 3,
        }
    );

    // Soft separation does not hard-fail on domain counts
    let soft_feasible = PlacementPlan::new("app.kubernetes.io/shard", "shard-0")
        .with_host_separation(HostSeparation::soft())
        .with_domain_counts(4, 3);
    assert!(soft_feasible.validate().is_ok());
}

#[test]
fn no_cloud_specific_field_in_rendered_json() {
    let plan = PlacementPlan::new("app.kubernetes.io/shard", "shard-0")
        .with_host_separation(HostSeparation::Hard)
        .with_zone_spread(ZoneSpread::required(1))
        .with_node_selector("topology.kubernetes.io/zone", "us-east1-b")
        .with_toleration(Toleration::equal(
            "node.kubernetes.io/unreachable",
            "true",
            "NoExecute",
        ));

    let affinity = plan.affinity().unwrap();
    let spread = plan.topology_spread_constraints().unwrap();
    let selector = plan.node_selector().unwrap();
    let tolerations = plan.tolerations().unwrap();

    let combined = json!({
        "affinity": affinity,
        "topologySpreadConstraints": spread,
        "nodeSelector": selector,
        "tolerations": tolerations,
    });

    assert_no_cloud_specific_fields(&combined);
}

fn assert_no_cloud_specific_fields(val: &Value) {
    let forbidden = [
        "ComputeClass",
        "computeclass",
        "compute_class",
        "compute-class",
        "NodePool",
        "nodepool",
        "node_pool",
        "node-pool",
        "n2-standard",
        "c2-standard",
        "e2-medium",
        "m5.large",
        "gke-",
        "eks.amazonaws.com",
    ];

    match val {
        Value::Object(map) => {
            for (k, v) in map {
                for f in &forbidden {
                    assert!(
                        !k.to_lowercase().contains(&f.to_lowercase()),
                        "found forbidden cloud term in key: {k}"
                    );
                }
                assert_no_cloud_specific_fields(v);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                assert_no_cloud_specific_fields(item);
            }
        }
        Value::String(s) => {
            for f in &forbidden {
                assert!(
                    !s.to_lowercase().contains(&f.to_lowercase()),
                    "found forbidden cloud term in value: {s}"
                );
            }
        }
        _ => {}
    }
}

#[test]
fn two_different_groups_render_selectors_that_do_not_match_each_other() {
    let group_a = PlacementPlan::new("app.kubernetes.io/shard", "shard-0")
        .with_host_separation(HostSeparation::Hard);
    let group_b = PlacementPlan::new("app.kubernetes.io/shard", "shard-1")
        .with_host_separation(HostSeparation::Hard);

    let aff_a = group_a.affinity().unwrap();
    let aff_b = group_b.affinity().unwrap();

    let sel_a = &aff_a["podAntiAffinity"]["requiredDuringSchedulingIgnoredDuringExecution"][0]
        ["labelSelector"]["matchLabels"];
    let sel_b = &aff_b["podAntiAffinity"]["requiredDuringSchedulingIgnoredDuringExecution"][0]
        ["labelSelector"]["matchLabels"];

    assert_eq!(sel_a, &json!({"app.kubernetes.io/shard": "shard-0"}));
    assert_eq!(sel_b, &json!({"app.kubernetes.io/shard": "shard-1"}));
    assert_ne!(sel_a, sel_b);

    // Pod labels for each shard
    let pod_labels_a = BTreeMap::from([
        ("app.kubernetes.io/name".to_string(), "database".to_string()),
        ("app.kubernetes.io/shard".to_string(), "shard-0".to_string()),
    ]);
    let pod_labels_b = BTreeMap::from([
        ("app.kubernetes.io/name".to_string(), "database".to_string()),
        ("app.kubernetes.io/shard".to_string(), "shard-1".to_string()),
    ]);

    // Group A anti-affinity matches Pod A, but NOT Pod B:
    assert!(selector_matches_labels(sel_a, &pod_labels_a));
    assert!(!selector_matches_labels(sel_a, &pod_labels_b));

    // Group B anti-affinity matches Pod B, but NOT Pod A:
    assert!(selector_matches_labels(sel_b, &pod_labels_b));
    assert!(!selector_matches_labels(sel_b, &pod_labels_a));

    // Thus, Group A and Group B may share a node.
}

#[test]
fn negative_control_removing_group_label_causes_different_groups_share_node_to_fail() {
    // If the anti-affinity selector omitted the group label and only matched the app name:
    let shared_app_selector_without_group = json!({
        "app.kubernetes.io/name": "database"
    });

    let pod_labels_a = BTreeMap::from([
        ("app.kubernetes.io/name".to_string(), "database".to_string()),
        ("app.kubernetes.io/shard".to_string(), "shard-0".to_string()),
    ]);
    let pod_labels_b = BTreeMap::from([
        ("app.kubernetes.io/name".to_string(), "database".to_string()),
        ("app.kubernetes.io/shard".to_string(), "shard-1".to_string()),
    ]);

    // Negative control verification:
    // Without group scoping, group A's selector matches both pod A and pod B,
    // which would incorrectly prevent shard-1 from sharing a node with shard-0!
    assert!(
        selector_matches_labels(&shared_app_selector_without_group, &pod_labels_a),
        "unscoped selector matches pod A"
    );
    assert!(
        selector_matches_labels(&shared_app_selector_without_group, &pod_labels_b),
        "negative control: removing group label causes selector to match other groups"
    );
}

fn selector_matches_labels(selector: &Value, labels: &BTreeMap<String, String>) -> bool {
    if let Value::Object(match_labels) = selector {
        for (k, v) in match_labels {
            if let Value::String(expected_val) = v {
                if labels.get(k) != Some(expected_val) {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    } else {
        false
    }
}
