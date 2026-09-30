//! The reconcile pass and its leader-gated, instrumented entry point.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kube::api::{Api, Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::runtime::events::EventType;
use kube::ResourceExt;

use super::children::{
    apply_object, cluster_children_finalizer, patch_finalizers, prune_cluster_scoped_object,
    prune_object, ready_replicas, ClusterPruneOutcome, PruneOutcome,
};
use super::{publish, Ctx, Error};
use crate::application::condition::status_patch::status_patch;
use crate::application::operator::managed_service::{ManagedService, ReadyFacts};

/// The instrumented entry point the controller actually calls (#2620).
///
/// The leader gate lives here rather than in [`reconcile`] on purpose: a
/// follower replica does no work, so counting its no-ops would inflate
/// `_reconcile_total` on the one replica that never touched the cluster and
/// dilute the error ratio of the one that did.
pub(super) async fn reconcile_entry<S: ManagedService>(
    obj: Arc<S>,
    ctx: Arc<Ctx>,
) -> Result<Action, Error> {
    if let Some(requeue) = ctx.leadership.follower_requeue() {
        return Ok(Action::requeue(requeue));
    }
    let started = Instant::now();
    let result = reconcile::<S>(obj, ctx.clone()).await;
    // Failures are timed too: a reconcile that fails after a 30s apiserver
    // timeout is a different problem from one that fails instantly, and a
    // histogram that only records successes cannot tell them apart.
    ctx.metrics.observe(started.elapsed());
    result
}

async fn reconcile<S: ManagedService>(obj: Arc<S>, ctx: Arc<Ctx>) -> Result<Action, Error> {
    let ns = obj
        .namespace()
        .ok_or(Error::Missing("metadata.namespace"))?;
    let name = obj.name_any();
    let client = &ctx.client;

    // Cluster-scoped children cannot use an owner reference to a namespaced
    // CR. Install a finalizer before the first apply, and remove only children
    // whose labels and field manager prove ownership.
    let cluster_children = obj.cluster_scoped_children();
    if !cluster_children.is_empty() {
        let finalizer = cluster_children_finalizer::<S>();
        let mut finalizers = obj.meta().finalizers.clone().unwrap_or_default();
        let api: Api<S> = Api::namespaced(client.clone(), &ns);
        if obj.meta().deletion_timestamp.is_some() {
            let mut pending = false;
            for target in &cluster_children {
                pending |= prune_cluster_scoped_object(client, S::MANAGER, target).await?
                    == ClusterPruneOutcome::DeleteRequested;
            }
            if pending {
                return Ok(Action::requeue(Duration::from_secs(1)));
            }
            if finalizers.iter().any(|item| item == &finalizer) {
                finalizers.retain(|item| item != &finalizer);
                patch_finalizers(&api, obj.as_ref(), finalizers).await?;
            }
            return Ok(Action::await_change());
        }
        if !finalizers.iter().any(|item| item == &finalizer) {
            finalizers.push(finalizer);
            patch_finalizers(&api, obj.as_ref(), finalizers).await?;
            return Ok(Action::await_change());
        }
    }

    let mut cluster_delete_pending = false;
    for target in cluster_children.iter().filter(|target| !target.desired) {
        cluster_delete_pending |= prune_cluster_scoped_object(client, S::MANAGER, target).await?
            == ClusterPruneOutcome::DeleteRequested;
    }
    if cluster_delete_pending {
        return Ok(Action::requeue(Duration::from_secs(1)));
    }

    // 1. Let the service perform async admission/observation, then apply the
    // planned children through the shared SSA path.
    let plan = obj
        .reconcile_plan(client.clone())
        .await
        .map_err(|error| Error::Plan(error.to_string()))?;
    for child in plan.children {
        apply_object(client, &ns, S::MANAGER, child).await?;
    }

    // 1b. Remove children a previous spec rendered and this one does not
    // (#2603). Server-side apply cannot express "this object should no longer
    // exist", so without this step a conditional child is opt-in only. Failing
    // here fails the reconcile on purpose: if the spec says an enforcement
    // object should be gone and we could not remove it, the CR has not
    // converged and its status must not claim otherwise.
    //
    // One ending is exempt from that (#3079). A target whose API the cluster
    // does not serve is a fact about the cluster, not a failure of this
    // operator, and aborting on it used to take the whole pass down *ahead* of
    // readiness observation and the status write — so the CR that most needed
    // to say something ended up saying nothing at all. Those targets are
    // collected here and reported as a condition below.
    let mut unavailable: Vec<String> = Vec::new();
    if let Some(uid) = obj.meta().uid.as_deref() {
        for target in obj.prunes() {
            if prune_object(client, &ns, uid, &target).await? == PruneOutcome::Unavailable {
                unavailable.push(format!(
                    "{} {}/{}",
                    target.api_version, target.kind, target.name
                ));
            }
        }
    }

    // 2. Observe readiness for the service's declared targets.
    let mut ready = HashMap::new();
    for t in obj.readiness_targets() {
        let r = ready_replicas(client, &ns, t.kind, &t.name).await?;
        ready.insert(t.name, r);
    }

    // 3. Write the status subresource (Merge avoids managed-field conflicts):
    // the service's status plus the conditions the application step stamps.
    let ready = ReadyFacts { ready };
    let status = status_patch(obj.as_ref(), &ready, &plan.context, &unavailable)?;

    // Tell the CR's owner, once, that their edit was picked up (#2620): only
    // for a generation the operator has not converged yet.
    if let Some(generation) = status.unconverged_generation() {
        publish(
            &ctx.recorder,
            obj.as_ref(),
            EventType::Normal,
            "Reconciled",
            "Reconcile",
            format!("applied spec generation {generation}"),
        )
        .await;
    }

    let api: Api<S> = Api::namespaced(client.clone(), &ns);
    api.patch_status(&name, &PatchParams::default(), &Patch::Merge(status.body()))
        .await?;

    // Periodic re-reconcile corrects drift and refreshes status.
    Ok(Action::requeue(Duration::from_secs(30)))
}
