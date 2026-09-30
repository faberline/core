//! The status subresource patch one reconcile pass writes: the service's own
//! status, plus the `status.conditions[]` this step stamps with the wall clock.

use serde_json::Value;

use super::now_rfc3339;
use crate::application::operator::managed_service::{ManagedService, ReadyFacts};
use crate::domain::condition::{project, ConditionFact, ConditionStatus};

/// The condition type the controller authors on its own behalf when a prune
/// target's API is not served (#3079).
///
/// Named rather than inlined because it is read back off the watched object as
/// well as written: `Patch::Merge` replaces a `conditions` array only when the
/// array is re-sent, so the pass that recovers has to recognise the block it
/// wrote on an earlier pass in order to clear it.
pub(crate) const PRUNE_BLOCKED: &str = "PruneBlocked";

/// The status patch for one reconcile pass.
#[derive(Debug)]
pub(crate) struct StatusPatch {
    body: Value,
    unconverged_generation: Option<i64>,
}

impl StatusPatch {
    /// The `{ "status": { … } }` body to send with `Patch::Merge`.
    pub(crate) fn body(&self) -> &Value {
        &self.body
    }

    /// The spec generation this pass applied, when the object had not yet
    /// converged on it. The controller announces it once, with a `Reconciled`
    /// Event. `None` when the generation had already converged, or when this
    /// pass wrote no conditions.
    pub(crate) fn unconverged_generation(&self) -> Option<i64> {
        self.unconverged_generation
    }
}

/// Why the conditions could not be written into the service's status patch.
#[derive(Debug)]
pub(crate) enum StatusPatchError {
    /// A part of the service's status patch that must be a JSON object is not.
    Missing(&'static str),
    /// The projected conditions did not serialize.
    Serde(serde_json::Error),
}

impl From<serde_json::Error> for StatusPatchError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serde(error)
    }
}

/// The service's status patch for the observed readiness and plan `context`,
/// with its `status.conditions[]` stamped (#2601, #3079).
///
/// `unavailable` names the prune targets whose API this cluster does not serve
/// (`"<apiVersion> <kind>/<name>"`); a non-empty list becomes the controller's
/// own `PruneBlocked` condition.
pub(crate) fn status_patch<S: ManagedService>(
    obj: &S,
    ready: &ReadyFacts,
    context: &Value,
    unavailable: &[String],
) -> Result<StatusPatch, StatusPatchError> {
    let mut body = obj.status_patch_with_context(ready, context);
    let mut unconverged_generation = None;

    // `status.conditions[]` (#2601). `lastTransitionTime` is a clock read,
    // which is why it cannot live in the service's synchronous, I/O-free status
    // projection — the service hands back clock-free facts and this step,
    // which the async reconcile loop runs, stamps them. Prior transition times
    // are carried forward from the watched object rather than left to the API
    // server: `Patch::Merge` replaces the array wholesale, so nothing survives
    // server-side unless it is re-sent.
    let facts = obj.conditions(ready, context);

    // One condition the *controller* authors rather than the service
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
        ConditionFact::new(
            PRUNE_BLOCKED,
            ConditionStatus::True,
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

        // Whether to tell the CR's owner, once, that their edit was picked up
        // (#2620). The controller publishes the Event; this step decides.
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
            unconverged_generation = Some(generation);
        }

        let now = now_rfc3339();
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
            project(&prior, facts, generation, &now)
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
            projected.extend(project(&prior, vec![fact], generation, &now));
        }
        body.as_object_mut()
            .ok_or(StatusPatchError::Missing("status patch root object"))?
            .entry("status")
            .or_insert_with(|| Value::Object(Default::default()))
            .as_object_mut()
            .ok_or(StatusPatchError::Missing("status patch `status` object"))?
            .insert("conditions".to_string(), serde_json::to_value(projected)?);
    }

    Ok(StatusPatch {
        body,
        unconverged_generation,
    })
}
