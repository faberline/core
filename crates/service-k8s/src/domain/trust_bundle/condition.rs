//! Condition and status projection for public trust bundle convergence.

use crate::domain::condition::{ConditionFact, ConditionStatus};
use std::collections::BTreeMap;

/// Name of the standard condition describing client trust bundle readiness.
pub const CLIENT_TRUST_READY_CONDITION: &str = "ClientTrustReady";

/// Per-namespace reconcile outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamespaceOutcome {
    /// Newly published or updated owned ConfigMap.
    Applied,
    /// Existing unowned ConfigMap was adopted.
    Adopted,
    /// ConfigMap was already current; no write needed.
    Unchanged,
    /// Owned ConfigMap in a pruned namespace was deleted.
    Pruned,
    /// Foreign ConfigMap in a pruned namespace was retained untouched.
    ForeignRetained,
    /// Adoption refused due to foreign ownership.
    AdoptionRefused { foreign_owner: String },
    /// Adoption refused because the existing ConfigMap has no owner and `adopt_unowned` is false.
    AdoptionRefusedUnowned,
    /// Operation in namespace failed with an error.
    Failed { error: String },
}

impl NamespaceOutcome {
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            Self::Applied | Self::Adopted | Self::Unchanged | Self::Pruned | Self::ForeignRetained
        )
    }
}

/// Compute the [`ConditionFact`] for `ClientTrustReady` from per-namespace outcomes.
pub fn client_trust_ready_condition(
    outcomes: &BTreeMap<String, NamespaceOutcome>,
    validation_error: Option<&str>,
) -> ConditionFact {
    if let Some(err) = validation_error {
        return ConditionFact::new(
            CLIENT_TRUST_READY_CONDITION,
            ConditionStatus::False,
            "InvalidCertificate",
            format!("certificate validation failed: {err}"),
        );
    }

    let mut failed_namespaces = Vec::new();
    let mut adoption_refusals = Vec::new();

    for (ns, outcome) in outcomes {
        match outcome {
            NamespaceOutcome::AdoptionRefused { foreign_owner } => {
                adoption_refusals.push(format!("{ns} (owned by {foreign_owner})"));
            }
            NamespaceOutcome::AdoptionRefusedUnowned => {
                adoption_refusals.push(format!("{ns} (unowned)"));
            }
            NamespaceOutcome::Failed { error } => {
                failed_namespaces.push(format!("{ns}: {error}"));
            }
            _ => {}
        }
    }

    if !adoption_refusals.is_empty() {
        return ConditionFact::new(
            CLIENT_TRUST_READY_CONDITION,
            ConditionStatus::False,
            "AdoptionRefused",
            format!(
                "adoption refused in namespaces: {}",
                adoption_refusals.join(", ")
            ),
        );
    }

    if !failed_namespaces.is_empty() {
        return ConditionFact::new(
            CLIENT_TRUST_READY_CONDITION,
            ConditionStatus::False,
            "ReconcileFailed",
            format!(
                "convergence failed in namespaces: {}",
                failed_namespaces.join(", ")
            ),
        );
    }

    ConditionFact::new(
        CLIENT_TRUST_READY_CONDITION,
        ConditionStatus::True,
        "Converged",
        "Public trust bundles converged across all allowed namespaces",
    )
}
