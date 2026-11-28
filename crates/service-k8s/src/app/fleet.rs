//! The Fleet controller loop: watches a Fleet custom resource (CR) in the
//! namespaces of a `WatchScope`, runs the leader-gated Fleet pass through the
//! kube adapter, writes the status and requeues.

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use kube::api::{Api, Patch, PatchParams};
use kube::runtime::controller::{Action, Controller};
use kube::runtime::watcher;
use kube::{Client, ResourceExt};
use serde_json::json;

use crate::api::fleet::{FleetSpec, FleetStatus};
use crate::application::condition::now_rfc3339;
use crate::application::fleet::{
    reconcile_fleet_under, FleetChildKind, FleetPassOutcome, FleetService,
};
use crate::application::operator::leadership::Leadership;
use crate::domain::condition::Condition;
use crate::domain::fleet::{FleetEntryPhase, FleetEntryStatus};
use crate::domain::leadership::{Election, LeaderLease};
use crate::infrastructure::fleet_cluster::KubeFleetCluster;
use crate::infrastructure::lease::KubeLease;
use crate::interfaces::operator::WatchScope;

use super::operator::{identity, lease_namespace};

/// Requeue of a Fleet whose entries are all ready: a low-frequency repair of drift.
pub const FLEET_REPAIR_REQUEUE: Duration = Duration::from_secs(300);
/// Requeue of a Fleet that still rolls out, and of a failed pass.
pub const FLEET_PROGRESS_REQUEUE: Duration = Duration::from_secs(15);

/// An error of one Fleet reconcile.
#[derive(Debug, thiserror::Error)]
pub enum FleetError {
    /// A kube API call failed.
    #[error("kube api error: {0}")]
    Kube(#[from] kube::Error),
    /// A value did not serialize.
    #[error("serialize error: {0}")]
    Serde(#[from] serde_json::Error),
    /// The Fleet object has no `metadata.uid`.
    #[error("fleet {0} has no metadata.uid")]
    MissingUid(String),
    /// The Fleet pass failed.
    #[error("fleet pass failed: {0}")]
    Pass(#[from] crate::application::fleet::FleetPassError),
    /// The blocking task of the pass failed.
    #[error("fleet pass task failed: {0}")]
    Task(String),
}

/// Run one pass for `obj` under `election`.
pub async fn reconcile_fleet_object<F: FleetService>(
    client: Client,
    obj: Arc<F>,
    election: Arc<Election>,
    scope: &WatchScope,
) -> Result<Action, FleetError> {
    // The leader gate is the first statement: a follower makes no request.
    let leadership = Leadership::held(election.clone());
    if let Some(requeue) = leadership.follower_requeue() {
        return Ok(Action::requeue(requeue));
    }

    let namespace = obj.namespace().unwrap_or_default();
    let name = obj.name_any();
    let uid = obj
        .meta()
        .uid
        .clone()
        .ok_or_else(|| FleetError::MissingUid(name.clone()))?;
    let now = now_rfc3339();
    let generation = obj.meta().generation.unwrap_or(0);
    let prior = obj.fleet_conditions().to_vec();

    if let Some(status) = unsupported_cluster_status(obj.fleet_spec(), &prior, generation, &now) {
        patch_status::<F>(&client, &namespace, &name, &status).await?;
        return Ok(Action::requeue(FLEET_REPAIR_REQUEUE));
    }

    let namespaces = match scope {
        WatchScope::AllNamespaces => None,
        WatchScope::Namespaces(list) => Some(list.clone()),
    };
    let kind = FleetChildKind {
        api_version: F::CHILD_API_VERSION.to_string(),
        kind: F::CHILD_KIND.to_string(),
        default_name: name.clone(),
    };
    let spec = obj.fleet_spec().clone();
    let cluster_client = client.clone();
    let handle = tokio::runtime::Handle::current();
    let manager = F::MANAGER;

    // The adapter blocks on the kube runtime, so the pass runs off the async workers.
    let outcome = tokio::task::spawn_blocking(move || {
        let mut cluster = KubeFleetCluster::new(cluster_client, handle, namespaces);
        reconcile_fleet_under(
            election,
            &mut cluster,
            &spec,
            &kind,
            &uid,
            manager,
            &prior,
            generation,
            &now,
        )
    })
    .await
    .map_err(|error| FleetError::Task(error.to_string()))??;

    match outcome {
        FleetPassOutcome::Requeue(delay) => Ok(Action::requeue(delay)),
        FleetPassOutcome::Ran(report) => {
            // The status is written even when an entry failed: the pass keeps per-entry errors in it.
            patch_status::<F>(&client, &namespace, &name, &report.status).await?;
            Ok(Action::requeue(requeue_after(&report.status)))
        }
    }
}

/// Run the Fleet controller for `F` until the process ends. It watches the
/// Fleet CR only, not its children: a child watch needs the child kind at
/// build time. Drift of a child is found by the requeue
/// (`FLEET_PROGRESS_REQUEUE` while rolling out, `FLEET_REPAIR_REQUEUE` after).
pub async fn run_fleet<F: FleetService>(scope: WatchScope) -> anyhow::Result<()> {
    let client = Client::try_default().await?;
    let lease = KubeLease::new(
        client.clone(),
        lease_namespace(F::MANAGER),
        F::MANAGER.to_string(),
    );
    let election = Election::new(identity(F::MANAGER));
    lease.start(election.clone());
    watch_fleet::<F>(client, election, scope).await;
    Ok(())
}

/// What each reconcile of a Fleet needs besides the object.
struct FleetCtx {
    client: Client,
    election: Arc<Election>,
    scope: WatchScope,
}

/// Watch `F` in `scope` and reconcile each object under `election`. With a
/// listed-namespace scope it makes no cluster-wide `list` or `watch` for `F`.
pub(crate) async fn watch_fleet<F: FleetService>(
    client: Client,
    election: Arc<Election>,
    scope: WatchScope,
) {
    match &scope {
        WatchScope::AllNamespaces => {
            tracing::info!(identity = %election.identity, manager = F::MANAGER, "fleet controller starting; watching CR cluster-wide");
        }
        WatchScope::Namespaces(list) => {
            tracing::info!(identity = %election.identity, manager = F::MANAGER, namespaces = ?list, "fleet controller starting; watching CR in listed namespaces");
        }
    }
    let ctx = Arc::new(FleetCtx {
        client: client.clone(),
        election,
        scope: scope.clone(),
    });
    let streams = fleet_apis::<F>(&client, &scope).into_iter().map(|api| {
        Controller::new(api, watcher::Config::default())
            .run(reconcile_entry::<F>, error_policy::<F>, ctx.clone())
            .boxed()
    });
    futures::stream::select_all(streams)
        .for_each(|res| async move {
            match res {
                Ok((obj, _)) => tracing::debug!(object = ?obj, "fleet reconciled"),
                Err(e) => tracing::warn!(error = %e, "fleet reconcile error"),
            }
        })
        .await;
}

/// One API handle for each watch that `scope` needs.
fn fleet_apis<F: FleetService>(client: &Client, scope: &WatchScope) -> Vec<Api<F>> {
    match scope {
        WatchScope::AllNamespaces => vec![Api::all(client.clone())],
        WatchScope::Namespaces(list) => list
            .iter()
            .map(|ns| Api::namespaced(client.clone(), ns))
            .collect(),
    }
}

async fn reconcile_entry<F: FleetService>(
    obj: Arc<F>,
    ctx: Arc<FleetCtx>,
) -> Result<Action, FleetError> {
    reconcile_fleet_object(ctx.client.clone(), obj, ctx.election.clone(), &ctx.scope).await
}

fn error_policy<F: FleetService>(obj: Arc<F>, err: &FleetError, _ctx: Arc<FleetCtx>) -> Action {
    tracing::warn!(error = %err, object = %obj.name_any(), "fleet reconcile failed");
    Action::requeue(FLEET_PROGRESS_REQUEUE)
}

/// Write the status subresource of one Fleet object.
async fn patch_status<F: FleetService>(
    client: &Client,
    namespace: &str,
    name: &str,
    status: &FleetStatus,
) -> Result<(), FleetError> {
    Api::<F>::namespaced(client.clone(), namespace)
        .patch_status(
            name,
            &PatchParams::default(),
            &Patch::Merge(json!({ "status": status })),
        )
        .await?;
    Ok(())
}

/// The requeue after a status: the repair time when every entry is ready.
fn requeue_after(status: &FleetStatus) -> Duration {
    let all_ready = !status.entries.is_empty()
        && status
            .entries
            .iter()
            .all(|entry| entry.phase == FleetEntryPhase::Ready);
    if all_ready {
        FLEET_REPAIR_REQUEUE
    } else {
        FLEET_PROGRESS_REQUEUE
    }
}

/// The status of a Fleet that names a remote cluster, or `None` when every
/// entry targets the control-plane cluster. A remote entry is reported as
/// `Unreachable` and nothing is applied to it.
fn unsupported_cluster_status<S>(
    spec: &FleetSpec<S>,
    prior: &[Condition],
    generation: i64,
    now: &str,
) -> Option<FleetStatus> {
    if spec
        .instances
        .iter()
        .all(|instance| instance.target.cluster_name() == "local")
    {
        return None;
    }
    let entries = spec
        .instances
        .iter()
        .map(|instance| {
            let cluster = instance.target.cluster_name();
            let (phase, message) = if cluster == "local" {
                (FleetEntryPhase::Pending, None)
            } else {
                (
                    FleetEntryPhase::Unreachable,
                    Some(format!(
                        "remote cluster targets are not supported yet: {cluster}"
                    )),
                )
            };
            FleetEntryStatus {
                target: instance.target.clone(),
                phase,
                observed_hash: None,
                message,
            }
        })
        .collect();
    Some(FleetStatus::project(prior, entries, generation, now))
}

#[cfg(test)]
mod tests;
