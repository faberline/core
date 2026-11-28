//! Fleet reconcile pass runs only while this replica holds the Lease (issue #659).

#![cfg(feature = "controller")]

use std::collections::BTreeSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use service_k8s::fleet::{AdoptionPolicy, ChildKey, FleetTarget, ObservedChild, PrunePolicy};
use service_k8s::{
    reconcile_fleet_under, Election, FleetChildKind, FleetCluster, FleetClusterError,
    FleetInstance, FleetPassOutcome, FleetRollout, FleetSpec,
};

#[derive(Default)]
struct FakeCluster {
    observed: Vec<ObservedChild>,
    fail_apply_for: Option<ChildKey>,
    calls: Vec<String>,
    /// Children that have rolled out and are ready.
    ready: BTreeSet<ChildKey>,
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

    fn is_ready(&mut self, key: &ChildKey) -> Result<bool, FleetClusterError> {
        Ok(self.ready.contains(key))
    }
}

fn kind() -> FleetChildKind {
    FleetChildKind {
        api_version: "apps/v1".into(),
        kind: "StatefulSet".into(),
        default_name: "svc".into(),
    }
}

fn key(name: &str) -> ChildKey {
    ChildKey::namespaced("apps/v1", "StatefulSet", "tenant-a", name)
}

/// One entry `a` in namespace `tenant-a`, with `Retain` pruning.
fn spec() -> FleetSpec<serde_json::Value> {
    FleetSpec {
        instances: vec![FleetInstance {
            target: FleetTarget::local("tenant-a"),
            name: Some("a".into()),
            spec: json!({"replicas": 1}),
        }],
        rollout: FleetRollout { max_concurrent: 1 },
        prune: PrunePolicy::Retain,
        adoption: AdoptionPolicy::Refuse,
    }
}

fn election(leads: bool) -> Arc<Election> {
    let election = Election::new("fleet-gate".to_string());
    election.is_leader.store(leads, Ordering::Relaxed);
    election
}

fn pass(
    election: Arc<Election>,
    cluster: &mut FakeCluster,
) -> Result<FleetPassOutcome, service_k8s::FleetPassError> {
    reconcile_fleet_under(
        election,
        cluster,
        &spec(),
        &kind(),
        "fleet-a",
        "faberline-fleet",
        &[],
        1,
        "2026-01-01T00:00:00Z",
    )
}

#[test]
fn leader_pass_applies_the_entry() {
    let mut cluster = FakeCluster::default();

    let outcome = pass(election(true), &mut cluster).expect("leader pass");

    let FleetPassOutcome::Ran(report) = outcome else {
        panic!("a leader must run the pass, got {outcome:?}");
    };
    assert_eq!(report.results.len(), 1);
    assert_eq!(
        cluster.calls,
        vec!["observed".to_string(), format!("apply {}", key("a"))],
    );
}

/// The negative test of #659: without the gate, a follower runs the pass and
/// this test fails.
#[test]
fn follower_pass_makes_no_write_and_requeues() {
    let mut cluster = FakeCluster::default();

    let outcome = pass(election(false), &mut cluster).expect("follower pass");

    assert!(
        matches!(outcome, FleetPassOutcome::Requeue(_)),
        "a follower must requeue, got {outcome:?}",
    );
    assert!(
        cluster.calls.is_empty(),
        "a follower must make no cluster call, got {:?}",
        cluster.calls,
    );
    assert!(
        !cluster
            .calls
            .iter()
            .any(|c| c.starts_with("apply") || c.starts_with("adopt") || c.starts_with("delete")),
        "a follower must make no write call, got {:?}",
        cluster.calls,
    );
}

#[test]
fn follower_requeue_delay_is_positive() {
    let mut cluster = FakeCluster::default();

    match pass(election(false), &mut cluster).expect("follower pass") {
        FleetPassOutcome::Requeue(delay) => assert!(delay > Duration::ZERO),
        other => panic!("a follower must requeue, got {other:?}"),
    }
}

#[test]
fn the_pass_does_not_make_a_follower_a_leader() {
    let mut cluster = FakeCluster::default();
    let election = election(false);

    pass(election.clone(), &mut cluster).expect("follower pass");

    assert!(!election.is_leader.load(Ordering::Relaxed));
}
