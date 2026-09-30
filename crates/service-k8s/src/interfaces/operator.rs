//! The shared operator: the watch/apply loop and the reconcile pass it drives.
//! The public entry points that start it live in the composition root.

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use kube::api::Api;
use kube::runtime::controller::{Action, Controller};
use kube::runtime::events::{Event, EventType, Recorder, Reporter};
use kube::runtime::watcher;
use kube::{Client, ResourceExt};

use crate::application::condition::status_patch::StatusPatchError;
use crate::application::operator::leadership::Leadership;
use crate::application::operator::managed_service::ManagedService;
use crate::interfaces::metrics::ControllerMetrics;
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
    leadership: Leadership,
    metrics: Arc<ControllerMetrics>,
    recorder: Recorder,
}

impl Ctx {
    /// The context `S` is reconciled with. Events are reported as
    /// `S::MANAGER`, from this replica's identity.
    fn new<S: ManagedService>(
        client: Client,
        leadership: Leadership,
        metrics: Arc<ControllerMetrics>,
    ) -> Self {
        let recorder = Recorder::new(
            client.clone(),
            Reporter {
                controller: S::MANAGER.to_string(),
                instance: Some(leadership.identity().to_string()),
            },
        );
        Self {
            client,
            leadership,
            metrics,
            recorder,
        }
    }
}

/// Watch every `S` cluster-wide and reconcile each under `leadership` until the
/// process is terminated. Every replica watches + reconciles, but only the
/// leader applies (HA-safe at `replicas > 1`).
pub(crate) async fn watch<S: ManagedService>(
    client: Client,
    leadership: Leadership,
    metrics: Arc<ControllerMetrics>,
) {
    let objs = Api::<S>::all(client.clone());
    tracing::info!(identity = %leadership.identity(), manager = S::MANAGER, "operator starting; watching CR cluster-wide");
    Controller::new(objs, watcher::Config::default())
        .run(
            reconcile_entry::<S>,
            error_policy::<S>,
            Arc::new(Ctx::new::<S>(client, leadership, metrics)),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _)) => tracing::debug!(object = ?obj, "reconciled"),
                Err(e) => tracing::warn!(error = %e, "reconcile error"),
            }
        })
        .await;
}

/// Exactly one reconcile pass for `obj` under `leadership`, through the same
/// instrumented entry point [`watch`] uses, with a metric set private to the
/// call.
pub(crate) async fn reconcile_one<S: ManagedService>(
    client: Client,
    obj: Arc<S>,
    leadership: Leadership,
) -> Result<Action, Error> {
    let metrics = Arc::new(ControllerMetrics::new(S::MANAGER));
    reconcile_entry::<S>(obj, Arc::new(Ctx::new::<S>(client, leadership, metrics))).await
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
