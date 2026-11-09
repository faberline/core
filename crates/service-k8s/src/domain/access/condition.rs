//! Kubernetes status condition projection for access grants.

use crate::domain::condition::{ConditionFact, ConditionStatus};

use super::{grant::AccessGrant, policy::validate_policy};

/// Name of the condition describing access policy readiness.
pub const ACCESS_POLICY_READY_CONDITION: &str = "AccessPolicyReady";

/// Per-grant outcome passed to [`access_policy_condition`].
///
/// An outcome is keyed by (namespace, name), the same key that
/// `validate_policy` uses for duplicates.
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

    pub fn applied(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(namespace, name, true)
    }

    pub fn failed(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(namespace, name, false)
    }

    pub fn grant(&self) -> &str {
        &self.name
    }
}

impl<N: Into<String>, G: Into<String>> From<(N, G, bool)> for GrantOutcome {
    fn from((namespace, name, applied): (N, G, bool)) -> Self {
        Self::new(namespace, name, applied)
    }
}

/// Compute the clock-free [`ConditionFact`] for the application's access policy.
///
/// Readiness is computed from the declared `grants`, not from `outcomes`:
/// a declared grant is applied only when it has an applied outcome with the
/// same (namespace, name) and no failed outcome with that key. A declared
/// grant with no outcome counts as failed. Outcomes that match no declared
/// grant are ignored.
///
/// Returns:
/// - `True / PolicyApplied` when every declared grant is applied,
/// - `True / DenyAll` when the desired set is empty,
/// - `False / InvalidPolicy` when validation failed (message names the grant),
/// - `False / ApplyFailed` when some grant is not applied (message lists only
///   those grants as `namespace/name`, never an error body),
///
/// and `True` again on subsequent calls once failures are resolved (recovery).
pub fn access_policy_condition(grants: &[AccessGrant], outcomes: &[GrantOutcome]) -> ConditionFact {
    if grants.is_empty() {
        return ConditionFact::new(
            ACCESS_POLICY_READY_CONDITION,
            ConditionStatus::True,
            "DenyAll",
            "deny-all: no access grants declared",
        );
    }

    if let Err(err) = validate_policy(grants) {
        return ConditionFact::new(
            ACCESS_POLICY_READY_CONDITION,
            ConditionStatus::False,
            "InvalidPolicy",
            format!("validation failed for grant '{}'", err.grant()),
        );
    }

    let failed_grants: Vec<String> = grants
        .iter()
        .filter(|grant| !is_applied(grant, outcomes))
        .map(|grant| format!("{}/{}", grant.namespace, grant.name))
        .collect();

    if !failed_grants.is_empty() {
        ConditionFact::new(
            ACCESS_POLICY_READY_CONDITION,
            ConditionStatus::False,
            "ApplyFailed",
            format!("apply failed for: {}", failed_grants.join(", ")),
        )
    } else {
        ConditionFact::new(
            ACCESS_POLICY_READY_CONDITION,
            ConditionStatus::True,
            "PolicyApplied",
            "all access grants applied",
        )
    }
}

/// A grant is applied only when an applied outcome exists for its key and no
/// failed outcome exists for the same key (fail closed).
fn is_applied(grant: &AccessGrant, outcomes: &[GrantOutcome]) -> bool {
    let matching = || {
        outcomes
            .iter()
            .filter(|o| o.namespace == grant.namespace && o.name == grant.name)
    };
    matching().any(|o| o.applied) && !matching().any(|o| !o.applied)
}
