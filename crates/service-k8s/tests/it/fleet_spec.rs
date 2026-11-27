//! Fleet spec shape: one engine serves many child kinds, plus the CRD schema hint and the status condition.

#![cfg(feature = "controller")]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use service_k8s::fleet::{
    plan_fleet, AdoptionPolicy, ChildKey, DesiredChild, FleetAction, FleetEntryPhase,
    FleetEntryStatus, FleetTarget, ObservedChild, PrunePolicy,
};
use service_k8s::{fleet_crd_schema_hint, FleetInstance, FleetRollout, FleetSpec, FleetStatus};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
struct DummyA {
    replicas: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
struct DummyB {
    flavor: String,
}

fn st(target: FleetTarget, phase: FleetEntryPhase) -> FleetEntryStatus {
    FleetEntryStatus {
        target,
        phase,
        observed_hash: None,
        message: None,
    }
}

#[test]
fn one_engine_shape_serves_two_child_kinds() {
    let spec_a = FleetSpec::<DummyA> {
        instances: vec![
            FleetInstance {
                target: FleetTarget::local("ns-a"),
                name: None,
                spec: DummyA { replicas: 1 },
            },
            FleetInstance {
                target: FleetTarget::new(Some("uat-1".into()), "ns-b"),
                name: None,
                spec: DummyA { replicas: 2 },
            },
        ],
        rollout: FleetRollout::default(),
        prune: PrunePolicy::default(),
        adoption: AdoptionPolicy::default(),
    };
    let spec_b = FleetSpec::<DummyB> {
        instances: vec![FleetInstance {
            target: FleetTarget::local("ns-a"),
            name: None,
            spec: DummyB { flavor: "x".into() },
        }],
        rollout: FleetRollout::default(),
        prune: PrunePolicy::default(),
        adoption: AdoptionPolicy::default(),
    };

    let entries_a = spec_a.entries().unwrap();
    let entries_b = spec_b.entries().unwrap();

    assert_eq!(entries_a.len(), 2);
    assert_eq!(entries_b.len(), 1);
    assert_eq!(entries_a[0].spec, json!({"replicas": 1}));
    assert_eq!(entries_a[1].spec, json!({"replicas": 2}));
    assert_eq!(
        entries_a[1].target,
        FleetTarget::new(Some("uat-1".into()), "ns-b")
    );
    assert_eq!(entries_b[0].spec, json!({"flavor": "x"}));
}

#[test]
fn default_prune_is_retain_and_adoption_is_refuse() {
    let spec: FleetSpec<DummyA> = serde_json::from_value(json!({"instances": []})).unwrap();

    assert_eq!(spec.prune, PrunePolicy::Retain);
    assert_eq!(spec.adoption, AdoptionPolicy::Refuse);
    assert_eq!(spec.rollout.max_concurrent, 1);
}

#[test]
fn removed_entry_is_retained_unless_delete_is_set() {
    let key = ChildKey::namespaced("v1", "ConfigMap", "ns-a", "x");
    let observed = vec![ObservedChild::owned(key.clone(), "h")];
    let desired: Vec<DesiredChild> = vec![];

    let retained = plan_fleet(
        desired.clone(),
        observed.clone(),
        AdoptionPolicy::Refuse,
        PrunePolicy::default(),
    )
    .unwrap();
    assert_eq!(
        retained.actions(),
        &[FleetAction::Orphan { key: key.clone() }]
    );

    let deleted = plan_fleet(
        desired,
        observed,
        AdoptionPolicy::Refuse,
        PrunePolicy::Delete,
    )
    .unwrap();
    assert_eq!(deleted.actions(), &[FleetAction::Prune { key }]);
}

#[test]
fn target_json_has_namespace_and_optional_cluster() {
    assert_eq!(
        serde_json::to_value(FleetTarget::local("ns-a")).unwrap(),
        json!({"namespace": "ns-a"})
    );
    assert_eq!(
        serde_json::to_value(FleetTarget::new(Some("uat-1".into()), "ns-b")).unwrap(),
        json!({"cluster": "uat-1", "namespace": "ns-b"})
    );

    let spec: FleetSpec<DummyA> = serde_json::from_value(json!({
        "instances": [{"target": {"namespace": "ns-a"}, "spec": {"replicas": 1}}]
    }))
    .unwrap();
    assert_eq!(spec.instances.len(), 1);
    assert_eq!(spec.instances[0].target, FleetTarget::local("ns-a"));
    assert_eq!(spec.instances[0].name, None);
}

#[test]
fn crd_schema_hint_is_structural() {
    let v = fleet_crd_schema_hint::<DummyA>("DummyA");

    assert_eq!(v["type"], "object");
    let props = v["properties"].as_object().unwrap();
    for key in ["instances", "rollout", "prune", "adoption"] {
        assert!(props.contains_key(key), "missing root property {key}");
    }
    let item_props = v["properties"]["instances"]["items"]["properties"]
        .as_object()
        .unwrap();
    for key in ["target", "name", "spec"] {
        assert!(item_props.contains_key(key), "missing item property {key}");
    }
    let target_props = v["properties"]["instances"]["items"]["properties"]["target"]["properties"]
        .as_object()
        .unwrap();
    assert!(target_props.contains_key("namespace"));
    assert!(target_props.contains_key("cluster"));
    assert!(v.get("x-kubernetes-preserve-unknown-fields").is_none());
    assert_eq!(v["description"], "Fleet of DummyA instances");
    assert!(!v.to_string().contains("uint32"));

    let other = fleet_crd_schema_hint::<DummyB>("DummyB");
    assert!(
        other["properties"]["instances"]["items"]["properties"]["spec"]["properties"]
            .as_object()
            .unwrap()
            .contains_key("flavor")
    );
}

#[test]
fn status_projects_one_ready_condition() {
    let not_ready = FleetStatus::project(
        &[],
        vec![
            st(FleetTarget::local("a"), FleetEntryPhase::Ready),
            st(FleetTarget::local("b"), FleetEntryPhase::Unreachable),
        ],
        3,
        "2026-11-26T00:00:00Z",
    );
    assert_eq!(not_ready.conditions.len(), 1);
    let c = &not_ready.conditions[0];
    assert_eq!(c.type_, "Ready");
    assert_eq!(c.status, "False");
    assert_eq!(c.reason, "EntriesNotReady");
    assert_eq!(c.message, "Unreachable entry local/b");
    assert_eq!(c.observed_generation, Some(3));
    assert_eq!(c.last_transition_time, "2026-11-26T00:00:00Z");

    let ready = FleetStatus::project(
        &[],
        vec![
            st(FleetTarget::local("a"), FleetEntryPhase::Ready),
            st(FleetTarget::local("b"), FleetEntryPhase::Ready),
        ],
        3,
        "2026-11-26T00:00:00Z",
    );
    assert_eq!(ready.conditions.len(), 1);
    assert_eq!(ready.conditions[0].status, "True");
    assert_eq!(ready.conditions[0].reason, "AllEntriesReady");
}

#[test]
fn status_json_is_camel_case() {
    assert_eq!(
        serde_json::to_value(FleetStatus::default()).unwrap(),
        json!({"entries": [], "conditions": []})
    );

    let with_hash = FleetEntryStatus {
        observed_hash: Some("h".into()),
        ..st(FleetTarget::local("a"), FleetEntryPhase::Ready)
    };
    let value = serde_json::to_value(&with_hash).unwrap();
    let obj = value.as_object().unwrap();
    assert!(obj.contains_key("observedHash"));
    assert!(!obj.contains_key("observed_hash"));

    let bare = st(FleetTarget::local("a"), FleetEntryPhase::Pending);
    let value = serde_json::to_value(&bare).unwrap();
    let obj = value.as_object().unwrap();
    assert!(!obj.contains_key("observedHash"));
    assert!(!obj.contains_key("message"));
}
