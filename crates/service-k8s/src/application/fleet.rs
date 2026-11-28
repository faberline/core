//! The Fleet reconcile pass: plan with `plan_fleet`, then apply each action
//! through a port. The port is synchronous; a fake implements it in tests.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::api::fleet::{FleetSpec, FleetStatus};
use crate::application::operator::leadership::Leadership;
use crate::domain::condition::Condition;
use crate::domain::fleet::{
    desired_hash, plan_fleet, plan_rollout_order, reject_duplicate_targets, ChildKey, DesiredChild,
    FleetAction, FleetEntryPhase, FleetEntryStatus, FleetPlanError, ObservedChild, RolloutState,
};
use crate::domain::leadership::Election;

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

    /// True when the child at `key` has rolled out and is ready.
    fn is_ready(&mut self, key: &ChildKey) -> Result<bool, FleetClusterError>;
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
    /// Per-entry status and the aggregate Ready condition.
    pub status: FleetStatus,
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

/// What the status shows for one entry. The pass fills it in while it runs.
#[derive(Clone, Debug)]
struct EntryOutcome {
    phase: FleetEntryPhase,
    observed_hash: Option<String>,
    message: Option<String>,
}

impl EntryOutcome {
    fn pending() -> Self {
        Self {
            phase: FleetEntryPhase::Pending,
            observed_hash: None,
            message: None,
        }
    }

    fn failed(error: &FleetClusterError) -> Self {
        Self {
            phase: FleetEntryPhase::Failed,
            observed_hash: None,
            message: Some(error.0.clone()),
        }
    }
}

/// The index of the entry (in declared order) whose child key is `key`.
fn entry_index(
    rendered: &[(ChildKey, serde_json::Value, String)],
    key: &ChildKey,
) -> Option<usize> {
    rendered.iter().position(|(k, _, _)| k == key)
}

/// Whether the action writes a child spec and so takes a rollout slot.
fn is_write(action: &FleetAction) -> bool {
    matches!(
        action,
        FleetAction::Create { .. } | FleetAction::Update { .. }
    )
}

/// Run one Fleet reconcile pass.
///
/// Duplicate targets stop the pass before any cluster call. A cluster error on
/// one action is stored in that action's result, and the pass goes on.
///
/// `Create` and `Update` run in the order of `plan_rollout_order`, within
/// `rollout.max_concurrent`. An entry whose predecessors are not ready waits for
/// the next pass. The report holds the projected `FleetStatus`.
// The three status inputs belong to FleetStatus::project.
#[allow(clippy::too_many_arguments)]
pub fn reconcile_fleet_once<S: Serialize>(
    cluster: &mut dyn FleetCluster,
    spec: &FleetSpec<S>,
    kind: &FleetChildKind,
    owner: &str,
    manager: &str,
    prior: &[Condition],
    observed_generation: i64,
    now: &str,
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

    // One slot per planned action, in plan order.
    let mut outcomes: Vec<Option<Result<(), FleetClusterError>>> =
        plan.actions.iter().map(|_| None).collect();
    // Rollout state and status per entry, in declared order.
    let mut states = vec![RolloutState::Ready; rendered.len()];
    let mut entry_outcomes = vec![EntryOutcome::pending(); rendered.len()];

    // First pass: everything except the writes. A write only marks its entry
    // as waiting; the rollout loop below applies it.
    for (a, action) in plan.actions.iter().enumerate() {
        match action {
            FleetAction::Create { key, .. } | FleetAction::Update { key, .. } => {
                match entry_index(&rendered, key) {
                    Some(idx) => states[idx] = RolloutState::Pending,
                    None => {
                        outcomes[a] = Some(Err(FleetClusterError(format!(
                            "no rendered spec for {key}"
                        ))));
                    }
                }
            }
            FleetAction::Adopt { key, desired_hash } => {
                let result = cluster.adopt(key, desired_hash, owner);
                if let Some(idx) = entry_index(&rendered, key) {
                    entry_outcomes[idx] = match &result {
                        Ok(()) => EntryOutcome::pending(),
                        Err(error) => EntryOutcome::failed(error),
                    };
                }
                outcomes[a] = Some(result);
            }
            FleetAction::Prune { key } => {
                outcomes[a] = Some(cluster.delete(key));
            }
            FleetAction::Unchanged { key } => {
                let probe = cluster.is_ready(key);
                if let Some(idx) = entry_index(&rendered, key) {
                    let hash = Some(rendered[idx].2.clone());
                    // A child that cannot be read keeps its slot, so the
                    // rollout does not start another entry past it.
                    let (state, outcome) = match probe {
                        Ok(true) => (
                            RolloutState::Ready,
                            EntryOutcome {
                                phase: FleetEntryPhase::Ready,
                                observed_hash: hash,
                                message: None,
                            },
                        ),
                        Ok(false) => (
                            RolloutState::Updating,
                            EntryOutcome {
                                phase: FleetEntryPhase::Applying,
                                observed_hash: hash,
                                message: None,
                            },
                        ),
                        Err(error) => (
                            RolloutState::Updating,
                            EntryOutcome {
                                phase: FleetEntryPhase::Failed,
                                observed_hash: hash,
                                message: Some(error.0),
                            },
                        ),
                    };
                    states[idx] = state;
                    entry_outcomes[idx] = outcome;
                }
                outcomes[a] = Some(Ok(()));
            }
            FleetAction::SkipForeign { key } => {
                if let Some(idx) = entry_index(&rendered, key) {
                    entry_outcomes[idx] = EntryOutcome {
                        phase: FleetEntryPhase::Failed,
                        observed_hash: None,
                        message: Some("child is not owned by this fleet".to_string()),
                    };
                }
                outcomes[a] = Some(Ok(()));
            }
            FleetAction::Orphan { .. } => {
                outcomes[a] = Some(Ok(()));
            }
        }
    }

    // Rollout: start the next pending entries while slots are free. A finished
    // apply frees its slot at once, so a failed entry does not hold the queue.
    let max = usize::try_from(spec.rollout.max_concurrent).unwrap_or(usize::MAX);
    while let Some(&idx) = plan_rollout_order(&states, max).first() {
        let key = rendered[idx].0.clone();
        let hash = rendered[idx].2.clone();
        let result = cluster.apply(&key, &rendered[idx].1, &hash, owner, manager);
        match &result {
            Ok(()) => {
                states[idx] = RolloutState::Updating;
                entry_outcomes[idx] = EntryOutcome {
                    phase: FleetEntryPhase::Applying,
                    observed_hash: Some(hash),
                    message: None,
                };
            }
            Err(error) => {
                states[idx] = RolloutState::Ready;
                entry_outcomes[idx] = EntryOutcome::failed(error);
            }
        }
        if let Some(a) = plan
            .actions
            .iter()
            .position(|action| is_write(action) && action.key() == &key)
        {
            outcomes[a] = Some(result);
        }
    }

    // One result per action, in plan order.
    let mut results = Vec::with_capacity(plan.actions.len());
    for (a, action) in plan.actions.into_iter().enumerate() {
        let error = outcomes[a]
            .take()
            .and_then(|outcome| outcome.err())
            .map(|err| err.0);
        results.push(FleetEntryResult { action, error });
    }

    let statuses: Vec<FleetEntryStatus> = entries
        .iter()
        .zip(entry_outcomes)
        .map(|(entry, outcome)| FleetEntryStatus {
            target: entry.target.clone(),
            phase: outcome.phase,
            observed_hash: outcome.observed_hash,
            message: outcome.message,
        })
        .collect();
    let status = FleetStatus::project(prior, statuses, observed_generation, now);

    Ok(FleetPassReport { results, status })
}

/// One Fleet custom resource that `run_fleet` reconciles.
#[cfg(feature = "controller")]
pub trait FleetService:
    kube::Resource<DynamicType = (), Scope = k8s_openapi::NamespaceResourceScope>
    + Clone
    + std::fmt::Debug
    + serde::de::DeserializeOwned
    + Send
    + Sync
    + 'static
{
    /// The child spec type of the instances.
    type Child: Serialize + Clone + Send + Sync + 'static;
    /// Field manager of the server-side apply and name of the leader-election Lease.
    const MANAGER: &'static str;
    /// `apiVersion` of the child objects, for example `apps/v1`.
    const CHILD_API_VERSION: &'static str;
    /// `kind` of the child objects, for example `StatefulSet`.
    const CHILD_KIND: &'static str;
    /// The desired state of this object.
    fn fleet_spec(&self) -> &FleetSpec<Self::Child>;
    /// The conditions of the last status; empty when there is none.
    fn fleet_conditions(&self) -> &[Condition];
}

/// The outcome of a leader-gated Fleet pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FleetPassOutcome {
    /// This replica leads: the pass ran and this is its report.
    Ran(FleetPassReport),
    /// This replica follows: nothing ran. Ask again after this delay.
    Requeue(Duration),
}

/// Run one Fleet pass only while `election` says this replica leads.
///
/// Like `reconcile_once` in `app/operator.rs`, it takes the election as a
/// parameter and never sets leadership itself: `Leadership::held` does not
/// write `is_leader`. The gate is the first statement, so a follower makes no
/// call on `cluster`, not even `observed`.
// The status inputs belong to reconcile_fleet_once.
#[allow(clippy::too_many_arguments)]
pub fn reconcile_fleet_under<S: Serialize>(
    election: Arc<Election>,
    cluster: &mut dyn FleetCluster,
    spec: &FleetSpec<S>,
    kind: &FleetChildKind,
    owner: &str,
    manager: &str,
    prior: &[Condition],
    observed_generation: i64,
    now: &str,
) -> Result<FleetPassOutcome, FleetPassError> {
    let leadership = Leadership::held(election);
    if let Some(requeue) = leadership.follower_requeue() {
        return Ok(FleetPassOutcome::Requeue(requeue));
    }
    reconcile_fleet_once(
        cluster,
        spec,
        kind,
        owner,
        manager,
        prior,
        observed_generation,
        now,
    )
    .map(FleetPassOutcome::Ran)
}
