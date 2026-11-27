//! Fleet reconcile pass against a fake `FleetCluster` port (issue #657).

#![cfg(feature = "controller")]

use serde_json::json;
use service_k8s::fleet::{
    desired_hash, AdoptionPolicy, ChildKey, FleetAction, FleetPlanError, FleetTarget,
    ObservedChild, PrunePolicy,
};
use service_k8s::{
    reconcile_fleet_once, FleetChildKind, FleetCluster, FleetClusterError, FleetInstance,
    FleetPassError, FleetRollout, FleetSpec,
};

#[derive(Default)]
struct FakeCluster {
    observed: Vec<ObservedChild>,
    fail_apply_for: Option<ChildKey>,
    calls: Vec<String>,
}

impl FleetCluster for FakeCluster {
    fn observed(
        &mut self,
        _owner: &str,
        _keys: &[ChildKey],
    ) -> Result<Vec<ObservedChild>, FleetClusterError> {
        self.calls.push("observed".into());
        Ok(std::mem::take(&mut self.observed))
    }

    fn apply(
        &mut self,
        key: &ChildKey,
        _spec: &serde_json::Value,
        _desired_hash: &str,
        _owner: &str,
        _manager: &str,
    ) -> Result<(), FleetClusterError> {
        self.calls.push(format!("apply {key}"));
        if self.fail_apply_for.as_ref() == Some(key) {
            return Err(FleetClusterError("apply refused by fake".into()));
        }
        Ok(())
    }

    fn adopt(
        &mut self,
        key: &ChildKey,
        _desired_hash: &str,
        _owner: &str,
    ) -> Result<(), FleetClusterError> {
        self.calls.push(format!("adopt {key}"));
        Ok(())
    }

    fn delete(&mut self, key: &ChildKey) -> Result<(), FleetClusterError> {
        self.calls.push(format!("delete {key}"));
        Ok(())
    }
}

fn kind() -> FleetChildKind {
    FleetChildKind {
        api_version: "apps/v1".into(),
        kind: "StatefulSet".into(),
        default_name: "svc".into(),
    }
}

fn fleet(
    names: &[&str],
    adoption: AdoptionPolicy,
    prune: PrunePolicy,
) -> FleetSpec<serde_json::Value> {
    FleetSpec {
        instances: names
            .iter()
            .map(|name| FleetInstance {
                target: FleetTarget::local("tenant-a"),
                name: Some((*name).to_string()),
                spec: json!({"replicas": 1}),
            })
            .collect(),
        rollout: FleetRollout { max_concurrent: 1 },
        prune,
        adoption,
    }
}

fn key(name: &str) -> ChildKey {
    ChildKey::namespaced("apps/v1", "StatefulSet", "tenant-a", name)
}

fn hash_of_a() -> String {
    desired_hash(&json!({"replicas": 1}))
}

fn run(
    cluster: &mut FakeCluster,
    spec: &FleetSpec<serde_json::Value>,
) -> Result<service_k8s::FleetPassReport, FleetPassError> {
    reconcile_fleet_once(cluster, spec, &kind(), "fleet-a", "faberline-fleet")
}

#[test]
fn duplicate_target_is_rejected_before_any_write() {
    let mut cluster = FakeCluster::default();
    let spec = fleet(&["a", "a"], AdoptionPolicy::Adopt, PrunePolicy::Delete);

    let err = run(&mut cluster, &spec).unwrap_err();

    assert_eq!(
        err,
        FleetPassError::Plan(FleetPlanError::DuplicateTarget(key("a")))
    );
    assert!(cluster.calls.is_empty(), "calls: {:?}", cluster.calls);
}

#[test]
fn adopt_does_not_change_the_child_spec() {
    let mut cluster = FakeCluster {
        observed: vec![ObservedChild::unowned(key("a"))],
        ..Default::default()
    };
    let spec = fleet(&["a"], AdoptionPolicy::Adopt, PrunePolicy::Retain);

    let report = run(&mut cluster, &spec).unwrap();

    assert_eq!(
        cluster.calls,
        ["observed".to_string(), format!("adopt {}", key("a"))]
    );
    assert_eq!(report.results.len(), 1);
    assert!(matches!(
        report.results[0].action,
        FleetAction::Adopt { .. }
    ));
    assert_eq!(report.results[0].error, None);
}

#[test]
fn refuse_adoption_leaves_an_unowned_child_alone() {
    let mut cluster = FakeCluster {
        observed: vec![ObservedChild::unowned(key("a"))],
        ..Default::default()
    };
    let spec = fleet(&["a"], AdoptionPolicy::Refuse, PrunePolicy::Retain);

    let report = run(&mut cluster, &spec).unwrap();

    assert_eq!(cluster.calls, ["observed"]);
    assert!(matches!(
        report.results[0].action,
        FleetAction::SkipForeign { .. }
    ));
}

#[test]
fn delete_prune_removes_a_dropped_owned_child() {
    let mut cluster = FakeCluster {
        observed: vec![
            ObservedChild::owned(key("a"), hash_of_a()),
            ObservedChild::owned(key("b"), "old"),
        ],
        ..Default::default()
    };
    let spec = fleet(&["a"], AdoptionPolicy::Refuse, PrunePolicy::Delete);

    run(&mut cluster, &spec).unwrap();

    assert_eq!(
        cluster.calls,
        ["observed".to_string(), format!("delete {}", key("b"))]
    );
}

#[test]
fn retain_prune_keeps_a_dropped_owned_child() {
    let mut cluster = FakeCluster {
        observed: vec![
            ObservedChild::owned(key("a"), hash_of_a()),
            ObservedChild::owned(key("b"), "old"),
        ],
        ..Default::default()
    };
    let spec = fleet(&["a"], AdoptionPolicy::Refuse, PrunePolicy::Retain);

    let report = run(&mut cluster, &spec).unwrap();

    assert_eq!(cluster.calls, ["observed"]);
    assert!(report
        .results
        .iter()
        .any(|r| matches!(&r.action, FleetAction::Orphan { key: k } if *k == key("b"))));
}

#[test]
fn a_foreign_child_is_never_deleted() {
    let mut cluster = FakeCluster {
        observed: vec![ObservedChild::foreign(key("b"), "other")],
        ..Default::default()
    };
    let spec = fleet(&[], AdoptionPolicy::Refuse, PrunePolicy::Delete);

    let report = run(&mut cluster, &spec).unwrap();

    assert!(
        !cluster.calls.iter().any(|c| c.starts_with("delete")),
        "calls: {:?}",
        cluster.calls
    );
    assert!(matches!(
        report.results[0].action,
        FleetAction::SkipForeign { .. }
    ));
}

#[test]
fn a_failing_entry_does_not_block_the_others() {
    let mut cluster = FakeCluster {
        fail_apply_for: Some(key("a")),
        ..Default::default()
    };
    let spec = fleet(&["a", "b"], AdoptionPolicy::Refuse, PrunePolicy::Retain);

    let report = run(&mut cluster, &spec).unwrap();

    assert_eq!(
        cluster.calls,
        [
            "observed".to_string(),
            format!("apply {}", key("a")),
            format!("apply {}", key("b")),
        ]
    );
    assert_eq!(report.results.len(), 2);
    let first_error = report.results[0]
        .error
        .as_deref()
        .expect("first entry fails");
    assert!(
        first_error.contains("apply refused by fake"),
        "{first_error}"
    );
    assert_eq!(report.results[1].error, None);
}

#[test]
fn an_up_to_date_child_is_left_unchanged() {
    let mut cluster = FakeCluster {
        observed: vec![ObservedChild::owned(key("a"), hash_of_a())],
        ..Default::default()
    };
    let spec = fleet(&["a"], AdoptionPolicy::Refuse, PrunePolicy::Retain);

    let report = run(&mut cluster, &spec).unwrap();

    assert_eq!(cluster.calls, ["observed"]);
    assert!(matches!(
        report.results[0].action,
        FleetAction::Unchanged { .. }
    ));
}
