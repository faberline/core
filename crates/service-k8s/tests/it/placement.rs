use std::collections::BTreeMap;

use serde_json::{json, Value};
use service_k8s::render::{
    placement::{
        HostSeparation, PlacementPlan, Toleration, ZoneSpread, TOPOLOGY_KEY_HOSTNAME,
        TOPOLOGY_KEY_ZONE,
    },
    ContainerPlan, DeploymentPlan, PersistentVolumeClaimPlan, PodPlan, PodRuntimePolicy, RenderCtx,
    StatefulSetPlan, WorkloadPlan,
};

fn context() -> RenderCtx<'static> {
    RenderCtx::new(
        "demo",
        "demo-operator",
        "demo.axiom.dev/v1",
        "Demo",
        "sample",
        "observability",
    )
}

#[test]
fn statefulset_plan_renders_failure_domain_placement_fields() {
    let cx = context();

    let placement = PlacementPlan::new("app.kubernetes.io/shard", "shard-0")
        .with_host_separation(HostSeparation::Hard)
        .with_zone_spread(ZoneSpread::required(1))
        .with_node_selector("topology.kubernetes.io/zone", "us-central1-a")
        .with_node_selector("kubernetes.io/os", "linux")
        .with_toleration(Toleration::equal(
            "node.kubernetes.io/unreachable",
            "true",
            "NoExecute",
        ))
        .with_toleration(Toleration::exists("dedicated", "NoSchedule"));

    let (group_key, group_val) = placement.group_label();
    assert_eq!(group_key, "app.kubernetes.io/shard");
    assert_eq!(group_val, "shard-0");

    let runtime =
        PodRuntimePolicy::restricted("sample-service-account", placement.node_selector().unwrap())
            .with_affinity(placement.affinity().unwrap())
            .with_tolerations(placement.tolerations().unwrap())
            .with_topology_spread_constraints(placement.topology_spread_constraints().unwrap());

    let pod = PodPlan::new(
        "data-node",
        ContainerPlan::new("engine", "engine:1.0", vec!["serve".into()]),
        runtime,
    )
    .with_labels(BTreeMap::from([(
        group_key.to_string(),
        group_val.to_string(),
    )]));

    let mut plan = WorkloadPlan::new(&cx);
    plan.add_stateful_set(StatefulSetPlan::new(
        "sample-shard-0",
        "sample-headless",
        3,
        pod,
        PersistentVolumeClaimPlan::new("data", "data-node", "20Gi", "/var/data"),
    ));

    let objects = plan.render().expect("render workload with statefulset");
    assert_eq!(objects.len(), 1);
    let sts = &objects[0];
    assert_eq!(sts["kind"], "StatefulSet");

    let template_spec = &sts["spec"]["template"]["spec"];
    let template_labels = &sts["spec"]["template"]["metadata"]["labels"];

    // Group label present on template
    assert_eq!(template_labels["app.kubernetes.io/shard"], "shard-0");

    // Group-scoped hard host anti-affinity
    assert_eq!(
        template_spec["affinity"]["podAntiAffinity"]
            ["requiredDuringSchedulingIgnoredDuringExecution"],
        json!([{
            "labelSelector": {
                "matchLabels": {
                    "app.kubernetes.io/shard": "shard-0",
                }
            },
            "topologyKey": TOPOLOGY_KEY_HOSTNAME,
        }])
    );

    // Group-scoped required zone spread
    assert_eq!(
        template_spec["topologySpreadConstraints"],
        json!([{
            "maxSkew": 1,
            "topologyKey": TOPOLOGY_KEY_ZONE,
            "whenUnsatisfiable": "DoNotSchedule",
            "labelSelector": {
                "matchLabels": {
                    "app.kubernetes.io/shard": "shard-0",
                }
            }
        }])
    );

    // Node selector
    assert_eq!(
        template_spec["nodeSelector"],
        json!({
            "kubernetes.io/os": "linux",
            "topology.kubernetes.io/zone": "us-central1-a",
        })
    );

    // Tolerations
    assert_eq!(
        template_spec["tolerations"],
        json!([
            {
                "key": "node.kubernetes.io/unreachable",
                "operator": "Equal",
                "value": "true",
                "effect": "NoExecute",
            },
            {
                "key": "dedicated",
                "operator": "Exists",
                "effect": "NoSchedule",
            }
        ])
    );
}

#[test]
fn pod_plan_renders_soft_host_separation_and_preferred_zone_spread() {
    let cx = context();

    let placement = PlacementPlan::new("faberline.dev/replica-group", "group-cache")
        .with_host_separation(HostSeparation::Soft(80))
        .with_zone_spread(ZoneSpread::preferred(2))
        .with_node_selector("kubernetes.io/arch", "arm64");

    let (group_key, group_val) = placement.group_label();

    let runtime =
        PodRuntimePolicy::restricted("sample-service-account", placement.node_selector().unwrap())
            .with_affinity(placement.affinity().unwrap())
            .with_tolerations(placement.tolerations().unwrap())
            .with_topology_spread_constraints(placement.topology_spread_constraints().unwrap());

    let pod = PodPlan::new(
        "cache-node",
        ContainerPlan::new("cache", "cache:1.0", vec!["start".into()]),
        runtime,
    )
    .with_labels(BTreeMap::from([(
        group_key.to_string(),
        group_val.to_string(),
    )]));

    let mut plan = WorkloadPlan::new(&cx);
    plan.add_deployment(DeploymentPlan::new("sample-cache", 2, pod));

    let objects = plan.render().expect("render workload with deployment");
    assert_eq!(objects.len(), 1);
    let deploy = &objects[0];
    assert_eq!(deploy["kind"], "Deployment");

    let template_spec = &deploy["spec"]["template"]["spec"];
    let template_labels = &deploy["spec"]["template"]["metadata"]["labels"];

    assert_eq!(
        template_labels["faberline.dev/replica-group"],
        "group-cache"
    );

    // Soft anti-affinity
    assert_eq!(
        template_spec["affinity"]["podAntiAffinity"]
            ["preferredDuringSchedulingIgnoredDuringExecution"],
        json!([{
            "weight": 80,
            "podAffinityTerm": {
                "labelSelector": {
                    "matchLabels": {
                        "faberline.dev/replica-group": "group-cache",
                    }
                },
                "topologyKey": TOPOLOGY_KEY_HOSTNAME,
            }
        }])
    );

    // Preferred zone spread
    assert_eq!(
        template_spec["topologySpreadConstraints"],
        json!([{
            "maxSkew": 2,
            "topologyKey": TOPOLOGY_KEY_ZONE,
            "whenUnsatisfiable": "ScheduleAnyway",
            "labelSelector": {
                "matchLabels": {
                    "faberline.dev/replica-group": "group-cache",
                }
            }
        }])
    );
}

#[test]
fn different_groups_render_selectors_allowing_them_to_share_a_node() {
    let group_0 = PlacementPlan::new("app.kubernetes.io/shard", "shard-0")
        .with_host_separation(HostSeparation::Hard);
    let group_1 = PlacementPlan::new("app.kubernetes.io/shard", "shard-1")
        .with_host_separation(HostSeparation::Hard);

    let aff_0 = group_0.affinity().unwrap();
    let aff_1 = group_1.affinity().unwrap();

    let sel_0 = &aff_0["podAntiAffinity"]["requiredDuringSchedulingIgnoredDuringExecution"][0]
        ["labelSelector"]["matchLabels"];
    let sel_1 = &aff_1["podAntiAffinity"]["requiredDuringSchedulingIgnoredDuringExecution"][0]
        ["labelSelector"]["matchLabels"];

    assert_ne!(sel_0, sel_1);
    assert_eq!(sel_0, &json!({"app.kubernetes.io/shard": "shard-0"}));
    assert_eq!(sel_1, &json!({"app.kubernetes.io/shard": "shard-1"}));

    let pod_shard_0 = BTreeMap::from([
        (
            "app.kubernetes.io/name".to_string(),
            "core-store".to_string(),
        ),
        ("app.kubernetes.io/shard".to_string(), "shard-0".to_string()),
    ]);
    let pod_shard_1 = BTreeMap::from([
        (
            "app.kubernetes.io/name".to_string(),
            "core-store".to_string(),
        ),
        ("app.kubernetes.io/shard".to_string(), "shard-1".to_string()),
    ]);

    // Anti-affinity for shard-0 does NOT match shard-1 pods
    assert!(!matches_selector(sel_0, &pod_shard_1));
    // Anti-affinity for shard-1 does NOT match shard-0 pods
    assert!(!matches_selector(sel_1, &pod_shard_0));

    // Both shards can safely run on the same node without mutual exclusion
}

#[test]
fn negative_control_unscoped_selector_prevents_different_groups_from_sharing_node() {
    // Negative control:
    // If the anti-affinity selector were erroneously not group-scoped (e.g. matching all pods of the app),
    // then shard-1 would match shard-0's anti-affinity selector, and the "different groups may share a node"
    // invariant would fail.
    let unscoped_app_selector = json!({
        "app.kubernetes.io/name": "core-store"
    });

    let pod_shard_1 = BTreeMap::from([
        (
            "app.kubernetes.io/name".to_string(),
            "core-store".to_string(),
        ),
        ("app.kubernetes.io/shard".to_string(), "shard-1".to_string()),
    ]);

    // The unscoped selector collides with shard-1 pods, causing mutual exclusion across shards:
    assert!(
        matches_selector(&unscoped_app_selector, &pod_shard_1),
        "negative control confirms removing the group label makes different groups collide"
    );
}

#[test]
fn no_cloud_specific_fields_in_rendered_statefulset_manifest() {
    let cx = context();

    let placement = PlacementPlan::new("app.kubernetes.io/shard", "shard-0")
        .with_host_separation(HostSeparation::Hard)
        .with_zone_spread(ZoneSpread::required(1))
        .with_node_selector("topology.kubernetes.io/zone", "us-central1-a")
        .with_toleration(Toleration::equal(
            "node.kubernetes.io/unreachable",
            "true",
            "NoExecute",
        ));

    let runtime = PodRuntimePolicy::restricted("sample-sa", placement.node_selector().unwrap())
        .with_affinity(placement.affinity().unwrap())
        .with_tolerations(placement.tolerations().unwrap())
        .with_topology_spread_constraints(placement.topology_spread_constraints().unwrap());

    let pod = PodPlan::new("data", ContainerPlan::new("c", "img:1", vec![]), runtime);

    let mut plan = WorkloadPlan::new(&cx);
    plan.add_stateful_set(StatefulSetPlan::new(
        "sample-sts",
        "sample-headless",
        1,
        pod,
        PersistentVolumeClaimPlan::new("data", "data", "10Gi", "/data"),
    ));

    let objects = plan.render().unwrap();
    for obj in &objects {
        assert_no_cloud_specific_fields(obj);
    }
}

fn matches_selector(selector: &Value, labels: &BTreeMap<String, String>) -> bool {
    if let Value::Object(map) = selector {
        for (k, v) in map {
            if let Value::String(expected) = v {
                if labels.get(k) != Some(expected) {
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
                        "forbidden cloud term in key: {k}"
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
                    "forbidden cloud term in value: {s}"
                );
            }
        }
        _ => {}
    }
}
