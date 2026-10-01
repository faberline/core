//! The operator's public entry points. They build what only the composition
//! root may build (the kube `Client` and the Lease adapter) and hand it to the
//! controller loop in interfaces.

use std::sync::Arc;

use kube::runtime::controller::Action;
use kube::Client;

use crate::application::operator::leadership::Leadership;
use crate::application::operator::managed_service::ManagedService;
use crate::domain::leadership::Election;
use crate::infrastructure::lease::KubeLease;
use crate::interfaces::metrics::{self, ControllerMetrics};
use crate::interfaces::operator::{reconcile_one, watch, Error};

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
    let lease = KubeLease::new(
        client.clone(),
        lease_namespace(S::MANAGER),
        S::MANAGER.to_string(),
    );
    let leadership = Leadership::campaign(identity(S::MANAGER), &lease);

    // Control-plane observability (#2620). The scrape listener runs alongside
    // the controller rather than inside it: leadership is read at scrape time,
    // so a follower replica publishes an honest `_leader 0` instead of going
    // dark, and every replica is independently scrapeable.
    let controller_metrics = Arc::new(ControllerMetrics::new(S::MANAGER));
    {
        let leadership = leadership.clone();
        tokio::spawn(metrics::serve(
            metrics::metrics_addr(),
            controller_metrics.clone(),
            move || leadership.is_leader(),
        ));
    }

    watch::<S>(client, leadership, controller_metrics).await;
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
    reconcile_one::<S>(client, obj, Leadership::held(election)).await
}
