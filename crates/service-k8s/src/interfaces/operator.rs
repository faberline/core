//! The shared operator: the watch/apply/lease loop and the service contract it drives.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use kube::api::Api;
use kube::runtime::controller::{Action, Controller};
use kube::runtime::events::{Event, EventType, Recorder, Reporter};
use kube::runtime::watcher;
use kube::{Client, ResourceExt};

use crate::application::condition::status_patch::StatusPatchError;
use crate::application::operator::managed_service::ManagedService;
use crate::infrastructure::lease::{self, Election};
use crate::interfaces::metrics::{self, ControllerMetrics};
use reconcile::reconcile_entry;

mod children;
mod reconcile;
#[cfg(test)]
mod tests;

/// Reconcile errors: `kube` + serde failures plus a guard for malformed rendered
/// objects (an operator bug, not a cluster condition).
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("kube api error: {0}")]
    Kube(#[from] kube::Error),
    #[error("serialize error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("rendered object missing required field: {0}")]
    Missing(&'static str),
    #[error("service reconcile plan failed: {0}")]
    Plan(String),
    #[error("refusing to delete cluster-scoped {kind}/{name}: {reason}")]
    ClusterOwnership {
        kind: String,
        name: String,
        reason: String,
    },
}

impl From<StatusPatchError> for Error {
    fn from(error: StatusPatchError) -> Self {
        match error {
            StatusPatchError::Missing(field) => Self::Missing(field),
            StatusPatchError::Serde(error) => Self::Serde(error),
        }
    }
}

struct Ctx {
    client: Client,
    election: Arc<Election>,
    metrics: Arc<ControllerMetrics>,
    recorder: Recorder,
}

/// This replica's leader-election identity (pod name in k8s, else the manager).
fn identity(manager: &str) -> String {
    std::env::var("POD_NAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| manager.to_string())
}

/// The namespace the leader-election Lease lives in (the operator's own).
fn lease_namespace(manager: &str) -> String {
    std::env::var("POD_NAMESPACE").unwrap_or_else(|_| format!("{manager}-system"))
}

/// Run the operator for `S` until the process is terminated. Every replica
/// watches + reconciles, but only the Lease holder applies (HA-safe at
/// `replicas > 1`).
pub async fn run<S: ManagedService>() -> anyhow::Result<()> {
    let client = Client::try_default().await?;
    let election = Election::new(identity(S::MANAGER));
    lease::spawn(
        client.clone(),
        lease_namespace(S::MANAGER),
        S::MANAGER.to_string(),
        election.clone(),
    );

    // Control-plane observability (#2620). The scrape listener runs alongside
    // the controller rather than inside it: leadership is read at scrape time,
    // so a follower replica publishes an honest `_leader 0` instead of going
    // dark, and every replica is independently scrapeable.
    let controller_metrics = Arc::new(ControllerMetrics::new(S::MANAGER));
    {
        let election = election.clone();
        tokio::spawn(metrics::serve(
            metrics::metrics_addr(),
            controller_metrics.clone(),
            move || election.is_leader.load(Ordering::Relaxed),
        ));
    }
    let recorder = Recorder::new(
        client.clone(),
        Reporter {
            controller: S::MANAGER.to_string(),
            instance: Some(election.identity.clone()),
        },
    );

    let objs = Api::<S>::all(client.clone());
    tracing::info!(identity = %election.identity, manager = S::MANAGER, "operator starting; watching CR cluster-wide");
    Controller::new(objs, watcher::Config::default())
        .run(
            reconcile_entry::<S>,
            error_policy::<S>,
            Arc::new(Ctx {
                client,
                election,
                metrics: controller_metrics,
                recorder,
            }),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _)) => tracing::debug!(object = ?obj, "reconciled"),
                Err(e) => tracing::warn!(error = %e, "reconcile error"),
            }
        })
        .await;
    Ok(())
}

/// Run exactly one reconcile pass for `obj` against `client`, under `election`.
///
/// [`run`] builds its own `Client` from the ambient kubeconfig and never
/// returns, so the convergence sequence it drives — apply the planned children,
/// prune the ones the spec dropped, observe readiness, write status — had no
/// observation point outside this module. That is a hole in the crate's
/// contract rather than a testing convenience: the sequence is the whole of
/// what a service in the kit delegates here, and nothing outside the module
/// could watch it end to end.
///
/// This is that one pass, against a `Client` and an [`Election`] the caller
/// supplies. Leadership is a parameter rather than something this function
/// decides: the leader gate in `reconcile_entry` is what makes `replicas > 1`
/// safe, and a public entry point that stored `is_leader = true` into an
/// election of its own making was a second, unguarded way past it. A caller
/// that wants a leader's pass says so in one visible line at its own call site.
///
/// The metric set is private to the call, so counting here cannot disturb a
/// running operator's exposition. It runs through the same instrumented entry
/// point [`run`] does, so a pass observed here is the pass the operator
/// performs — the leader gate included, which is why a follower's election
/// yields a requeue and no cluster write at all.
pub async fn reconcile_once<S: ManagedService>(
    client: Client,
    obj: Arc<S>,
    election: Arc<Election>,
) -> Result<Action, Error> {
    let recorder = Recorder::new(
        client.clone(),
        Reporter {
            controller: S::MANAGER.to_string(),
            instance: Some(election.identity.clone()),
        },
    );
    let ctx = Arc::new(Ctx {
        client,
        election,
        metrics: Arc::new(ControllerMetrics::new(S::MANAGER)),
        recorder,
    });
    reconcile_entry::<S>(obj, ctx).await
}

/// Publish one Event against the CR, best-effort.
///
/// Best-effort is deliberate: an operator that fails its reconcile *and* then
/// fails to say so must still requeue and retry. Losing the narration is a
/// smaller harm than a controller that stops because its own event write was
/// rejected — and the failure is not silent, because the reconcile-error
/// counter has already moved and the log carries both errors.
///
/// Requires the `events.k8s.io` / `events` `create,patch` grant in the
/// operator's ClusterRole.
async fn publish<S: ManagedService>(
    recorder: &Recorder,
    obj: &S,
    type_: EventType,
    reason: &str,
    action: &str,
    note: String,
) {
    let event = Event {
        type_,
        reason: reason.to_string(),
        note: Some(note),
        action: action.to_string(),
        secondary: None,
    };
    if let Err(error) = recorder.publish(&event, &obj.object_ref(&())).await {
        tracing::warn!(%error, reason, "failed to publish event");
    }
}

/// What the controller does with a failed reconcile.
///
/// Until #2620 this discarded the error entirely and returned a bare requeue,
/// which made a CR failing every single round externally identical to one
/// converging fine: nothing counted the failure, and the object's owner was
/// never told. Both halves of that are fixed here — the counter feeds the
/// error-rate alert, the Event feeds `kubectl describe`.
fn error_policy<S: ManagedService>(obj: Arc<S>, err: &Error, ctx: Arc<Ctx>) -> Action {
    ctx.metrics.observe_error();
    tracing::warn!(
        error = %err,
        object = %obj.name_any(),
        namespace = obj.namespace().unwrap_or_default(),
        "reconcile failed"
    );

    // `error_policy` is synchronous by the controller's contract, so the write
    // is detached. The `Recorder`'s 6-minute dedup window collapses a
    // repeatedly failing reconcile into one counted series rather than a flood.
    let recorder = ctx.recorder.clone();
    let note = err.to_string();
    tokio::spawn(async move {
        publish(
            &recorder,
            obj.as_ref(),
            EventType::Warning,
            "ReconcileFailed",
            "Reconcile",
            note,
        )
        .await;
    });

    Action::requeue(Duration::from_secs(15))
}
