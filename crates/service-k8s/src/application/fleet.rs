//! The Fleet reconcile pass: plan with `plan_fleet`, then apply each action
//! through a port. The port is synchronous; a fake implements it in tests.

use serde::Serialize;

use crate::api::fleet::FleetSpec;
use crate::domain::fleet::{
    desired_hash, plan_fleet, reject_duplicate_targets, ChildKey, DesiredChild, FleetAction,
    FleetPlanError, ObservedChild,
};

/// The child kind that a Fleet manages, for example `apps/v1` and `StatefulSet`.
/// `default_name` is the name of an entry that has no `name`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetChildKind {
    pub api_version: String,
    pub kind: String,
    pub default_name: String,
}

/// An error from the cluster port, as text.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct FleetClusterError(pub String);

/// What the Fleet pass needs from the cluster. Synchronous, as `ReshardCluster`.
pub trait FleetCluster {
    /// Children that exist now: every child that carries `owner` in the label
    /// `faberline.dev/fleet-owner`, plus every existing object at one of `keys`
    /// (unowned or foreign included).
    fn observed(
        &mut self,
        owner: &str,
        keys: &[ChildKey],
    ) -> Result<Vec<ObservedChild>, FleetClusterError>;

    /// Server-side apply `spec` at `key` as the field manager `manager`. Stamp the
    /// owner label and the desired-hash annotation.
    fn apply(
        &mut self,
        key: &ChildKey,
        spec: &serde_json::Value,
        desired_hash: &str,
        owner: &str,
        manager: &str,
    ) -> Result<(), FleetClusterError>;

    /// Stamp the owner label and the desired-hash annotation on an existing unowned
    /// object. The spec of the object does not change.
    fn adopt(
        &mut self,
        key: &ChildKey,
        desired_hash: &str,
        owner: &str,
    ) -> Result<(), FleetClusterError>;

    /// Delete the child at `key`.
    fn delete(&mut self, key: &ChildKey) -> Result<(), FleetClusterError>;
}

/// The outcome of one planned action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetEntryResult {
    pub action: FleetAction,
    /// `None` when the action succeeded or wrote nothing.
    pub error: Option<String>,
}

/// One result per planned action, in plan order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetPassReport {
    pub results: Vec<FleetEntryResult>,
}

/// An error that stops the whole pass before or during the planning step.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FleetPassError {
    #[error(transparent)]
    Plan(#[from] FleetPlanError),
    #[error("entry does not serialize: {0}")]
    Render(String),
    #[error("cluster read failed: {0}")]
    Cluster(FleetClusterError),
}

/// Run one Fleet reconcile pass.
///
/// Duplicate targets stop the pass before any cluster call. A cluster error on
/// one action is stored in that action's result, and the pass goes on.
pub fn reconcile_fleet_once<S: Serialize>(
    cluster: &mut dyn FleetCluster,
    spec: &FleetSpec<S>,
    kind: &FleetChildKind,
    owner: &str,
    manager: &str,
) -> Result<FleetPassReport, FleetPassError> {
    let entries = spec
        .entries()
        .map_err(|err| FleetPassError::Render(err.to_string()))?;
    reject_duplicate_targets(&entries, &kind.api_version, &kind.kind, &kind.default_name)?;

    let mut rendered: Vec<(ChildKey, serde_json::Value, String)> =
        Vec::with_capacity(entries.len());
    for entry in &entries {
        let key = entry.child_key(&kind.api_version, &kind.kind, &kind.default_name);
        let hash = desired_hash(&entry.spec);
        rendered.push((key, entry.spec.clone(), hash));
    }

    let desired = rendered
        .iter()
        .map(|(key, _, hash)| DesiredChild::new(key.clone(), hash.clone()));
    let keys: Vec<ChildKey> = rendered.iter().map(|(key, _, _)| key.clone()).collect();
    let observed = cluster
        .observed(owner, &keys)
        .map_err(FleetPassError::Cluster)?;

    let plan = plan_fleet(desired, observed, spec.adoption, spec.prune)?;

    let mut results = Vec::with_capacity(plan.actions.len());
    for action in plan.actions {
        let outcome = match &action {
            FleetAction::Create { key, desired_hash }
            | FleetAction::Update { key, desired_hash } => {
                match rendered.iter().find(|(k, _, _)| k == key) {
                    Some((_, child_spec, _)) => {
                        cluster.apply(key, child_spec, desired_hash, owner, manager)
                    }
                    None => Err(FleetClusterError(format!("no rendered spec for {key}"))),
                }
            }
            FleetAction::Adopt { key, desired_hash } => cluster.adopt(key, desired_hash, owner),
            FleetAction::Prune { key } => cluster.delete(key),
            FleetAction::Unchanged { .. }
            | FleetAction::SkipForeign { .. }
            | FleetAction::Orphan { .. } => Ok(()),
        };
        results.push(FleetEntryResult {
            error: outcome.err().map(|err| err.0),
            action,
        });
    }

    Ok(FleetPassReport { results })
}
