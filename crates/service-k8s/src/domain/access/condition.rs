//! Kubernetes status condition projection for access grants.

use crate::domain::condition::{ConditionFact, ConditionStatus};

use super::{grant::AccessGrant, policy::validate_policy};

/// Name of the condition describing access policy readiness.
pub const ACCESS_POLICY_READY_CONDITION: &str = "AccessPolicyReady";

/// Per-grant outcome passed to [`access_policy_condition`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantOutcome {
    pub name: String,
    pub applied: bool,
}

impl GrantOutcome {
    pub fn new(name: impl Into<String>, applied: bool) -> Self {
        Self {
            name: name.into(),
            applied,
        }
    }

    pub fn applied(name: impl Into<String>) -> Self {
        Self::new(name, true)
    }

    pub fn failed(name: impl Into<String>) -> Self {
        Self::new(name, false)
    }

    pub fn grant(&self) -> &str {
        &self.name
    }
}

impl<S: Into<String>> From<(S, bool)> for GrantOutcome {
    fn from((name, applied): (S, bool)) -> Self {
        Self::new(name, applied)
    }
}

/// Compute the clock-free [`ConditionFact`] for the application's access policy.
///
/// Returns:
/// - `True / PolicyApplied` when every grant is applied,
/// - `True / DenyAll` when the desired set is empty,
/// - `False / InvalidPolicy` when validation failed (message names the grant),
/// - `False / ApplyFailed` when some grant failed (message lists the failed grant names, never an error body),
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

    let failed_grants: Vec<&str> = outcomes
        .iter()
        .filter(|o| !o.applied)
        .map(|o| o.name.as_str())
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
