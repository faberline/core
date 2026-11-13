//! The `AccessPolicyReady` condition: what a policy and the outcomes of
//! applying it say about readiness. Pure: the caller supplies the outcomes.

use crate::domain::condition::{ConditionFact, ConditionStatus};

use super::{validate_policy, AccessGrant};

/// The `type` of the condition this module computes.
pub const ACCESS_POLICY_READY_CONDITION: &str = "AccessPolicyReady";

/// The result of applying one grant, keyed by its (namespace, name).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantOutcome {
    pub namespace: String,
    pub name: String,
    pub applied: bool,
}

impl GrantOutcome {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>, applied: bool) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
            applied,
        }
    }

    /// The grant was applied.
    pub fn applied(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(namespace, name, true)
    }

    /// Applying the grant failed.
    pub fn failed(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(namespace, name, false)
    }
}

/// What one reconcile pass did for one grant (or one stale candidate).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessOutcome {
    Applied,
    Adopted,
    Pruned,
    ForeignRetained,
    OwnerConflict { foreign_owner: String },
    AdoptionRefusedUnowned,
    Failed { error: String },
}

impl AccessOutcome {
    /// `true` for `Applied` and `Adopted` only.
    pub fn is_applied(&self) -> bool {
        matches!(self, Self::Applied | Self::Adopted)
    }
}

/// Compute the `AccessPolicyReady` condition for `grants` and their `outcomes`.
///
/// An empty `grants` is a deny-all and is ready. A policy that fails
/// validation is not ready. Otherwise every declared grant must have an
/// applied outcome and no failed one; a grant with no outcome is not applied.
pub fn access_policy_condition(grants: &[AccessGrant], outcomes: &[GrantOutcome]) -> ConditionFact {
    let fact = |status, reason: &str, message: String| {
        ConditionFact::new(ACCESS_POLICY_READY_CONDITION, status, reason, message)
    };

    if grants.is_empty() {
        return fact(
            ConditionStatus::True,
            "DenyAll",
            "deny-all: no access grants declared".into(),
        );
    }
    if let Err(e) = validate_policy(grants) {
        return fact(
            ConditionStatus::False,
            "InvalidPolicy",
            format!("validation failed for grant '{}'", e.grant()),
        );
    }

    let not_applied: Vec<String> = grants
        .iter()
        .filter(|grant| !is_applied(grant, outcomes))
        .map(|grant| format!("{}/{}", grant.namespace, grant.name))
        .collect();
    if !not_applied.is_empty() {
        return fact(
            ConditionStatus::False,
            "ApplyFailed",
            format!("apply failed for: {}", not_applied.join(", ")),
        );
    }

    fact(
        ConditionStatus::True,
        "PolicyApplied",
        "all access grants applied".into(),
    )
}

/// A grant is applied when at least one outcome for its key says so and none
/// says it failed.
fn is_applied(grant: &AccessGrant, outcomes: &[GrantOutcome]) -> bool {
    let matching: Vec<&GrantOutcome> = outcomes
        .iter()
        .filter(|o| o.namespace == grant.namespace && o.name == grant.name)
        .collect();
    matching.iter().any(|o| o.applied) && matching.iter().all(|o| o.applied)
}
