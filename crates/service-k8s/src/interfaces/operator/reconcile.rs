//! The reconcile pass and its leader-gated, instrumented entry point.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kube::api::{Api, Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::runtime::events::EventType;
use kube::ResourceExt;
use serde_json::Value;

use super::children::{
    apply_object, cluster_children_finalizer, patch_finalizers, prune_cluster_scoped_object,
    prune_object, ready_replicas, ClusterPruneOutcome, PruneOutcome, PRUNE_BLOCKED,
};
use super::{publish, Ctx, Error};
use crate::service::{self, ManagedService, ReadyFacts};

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
    if !ctx.election.is_leader.load(Ordering::Relaxed) {
        return Ok(Action::requeue(Duration::from_secs(10)));
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

    // 3. Write the status subresource (Merge avoids managed-field conflicts).
    let ready = ReadyFacts { ready };
    let mut status = obj.status_patch_with_context(&ready, &plan.context);

    // 3b. `status.conditions[]` (#2601). `lastTransitionTime` is a clock read,
    // which is why it cannot live in the service's synchronous, I/O-free status
    // projection — the service hands back clock-free facts and the reconcile
    // loop, already async, stamps them. Prior transition times are carried
    // forward from the watched object rather than left to the API server:
    // `Patch::Merge` replaces the array wholesale, so nothing survives
    // server-side unless it is re-sent.
    let facts = obj.conditions(&ready, &plan.context);

    // 3c. One condition the *controller* authors rather than the service
    // (#3079). No service can report this one: the prune GET is the
    // controller's own call and a service has no way to learn that it failed.
    // Without it, a CR whose enforcement object is still up because its API
    // vanished reports a perfectly healthy status and nobody is told.
    //
    // Kept separate from the service's `facts` rather than pushed onto them:
    // "did the service declare anything this pass" is a question the write
    // below has to answer, and folding the controller's own condition into the
    // same vector makes it unanswerable.
    let blocked = (!unavailable.is_empty()).then(|| {
        service::ConditionFact::new(
            PRUNE_BLOCKED,
            service::ConditionStatus::True,
            "ApiNotServed",
            format!(
                "this cluster does not serve the API for {}; the prune retries \
                 on each reconcile and completes once the API appears",
                unavailable.join(", ")
            ),
        )
    });

    let prior = obj.observed_conditions();
    // The gate reads `facts` after the controller has added its own, so a
    // service that declares no conditions still carries `PruneBlocked` — a
    // gate keyed on the service's own facts would drop precisely the round
    // that had something to say.
    //
    // It also fires on a `PruneBlocked` a previous pass wrote, even when this
    // pass has no facts at all. `Patch::Merge` replaces an array only when the
    // array is re-sent, so skipping the block here would leave the CR
    // reporting a block against an API that has since come back, permanently.
    // A service with no conditions and nothing blocked still takes neither
    // branch, and writes exactly the status shape it wrote before #3079.
    if !facts.is_empty() || blocked.is_some() || prior.iter().any(|c| c.type_ == PRUNE_BLOCKED) {
        let generation = obj.meta().generation.unwrap_or(0);

        // Tell the CR's owner, once, that their edit was picked up (#2620).
        //
        // The trigger is a generation the operator has not converged yet, which
        // makes the event fire on the first reconcile of a new CR and on every
        // spec change, and stay silent through the 30s steady-state requeues in
        // between. Publishing unconditionally would instead mean one apiserver
        // write per CR per requeue forever, deduplicated into an ever-counting
        // `EventSeries` that says nothing.
        let converged = prior
            .iter()
            .filter_map(|c| c.observed_generation)
            .max()
            .is_some_and(|observed| observed == generation);
        if !converged {
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

        let now = service::now_rfc3339();
        // The service's half. A service declares its whole condition set every
        // pass, so a declared set replaces the array wholesale — that is what
        // `project` is for, and a condition the service stopped declaring is
        // meant to disappear.
        //
        // A pass where it declared *nothing* is not that. It is a pass with no
        // opinion — readiness the pass could not observe, most often the same
        // cluster hiccup that made an API stop being served — and the only
        // reason control reached here is the controller's own condition. The
        // controller authors exactly one condition and so may remove exactly
        // one: everything else in the array is carried forward as found,
        // transition times and observed generations included. Replacing it with
        // the empty projection would delete the service's status on the pass
        // that was supposed to withdraw the controller's.
        let mut projected: Vec<_> = if facts.is_empty() {
            prior.to_vec()
        } else {
            service::project(&prior, facts, generation, &now)
        };
        // `PruneBlocked` is the controller's condition and the controller is its
        // sole author, so it is stripped here whatever produced it — the
        // carried-forward `prior`, or a service that declared a fact of the same
        // name. `project` maps facts 1:1 with no dedup, so without this the
        // second case appends a duplicate: two entries with the same `type` in
        // `status.conditions`, which the CRD declares `listType: map` keyed on
        // `type` and the apiserver refuses outright — costing the whole status
        // patch, not just the condition.
        projected.retain(|c| c.type_ != PRUNE_BLOCKED);
        if let Some(fact) = blocked {
            projected.extend(service::project(&prior, vec![fact], generation, &now));
        }
        status
            .as_object_mut()
            .ok_or(Error::Missing("status patch root object"))?
            .entry("status")
            .or_insert_with(|| Value::Object(Default::default()))
            .as_object_mut()
            .ok_or(Error::Missing("status patch `status` object"))?
            .insert("conditions".to_string(), serde_json::to_value(projected)?);
    }

    let api: Api<S> = Api::namespaced(client.clone(), &ns);
    api.patch_status(&name, &PatchParams::default(), &Patch::Merge(&status))
        .await?;

    // Periodic re-reconcile corrects drift and refreshes status.
    Ok(Action::requeue(Duration::from_secs(30)))
}
