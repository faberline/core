//! Reconcile access grants: apply, adopt, refuse on owner conflict, prune stale grants.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::domain::access::{
    access_policy_condition, validate_policy, AccessGrant, AccessObjectKind, AccessOutcome,
    AccessStore, GrantOutcome,
};
use crate::domain::condition::{ConditionFact, ConditionStatus};
use crate::domain::fleet::Ownership;
use crate::domain::trust_bundle::OWNER_IDENTITY_LABEL;
use crate::infrastructure::manifest::render_access_grants;

/// The result of one access reconcile pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessReport {
    /// Keyed by (namespace, grant name): declared grants and stale candidates.
    pub outcomes: BTreeMap<(String, String), AccessOutcome>,
    /// `AccessPolicyReady`, from `access_policy_condition`.
    pub condition: ConditionFact,
    pub is_ready: bool,
}

fn report(
    outcomes: BTreeMap<(String, String), AccessOutcome>,
    grants: &[AccessGrant],
    grant_outcomes: &[GrantOutcome],
) -> AccessReport {
    let condition = access_policy_condition(grants, grant_outcomes);
    let is_ready = condition.status == ConditionStatus::True;
    AccessReport {
        outcomes,
        condition,
        is_ready,
    }
}

fn ownership(labels: &BTreeMap<String, String>, owner: &str) -> Ownership {
    Ownership::from_owner(labels.get(OWNER_IDENTITY_LABEL).map(String::as_str), owner)
}

/// Apply one declared grant: refuse foreign and unowned objects, adopt when allowed.
async fn apply_grant(
    store: &dyn AccessStore,
    owner: &str,
    namespace: &str,
    name: &str,
    role: Value,
    binding: Value,
    adopt_unowned: bool,
) -> AccessOutcome {
    let role_labels = match store.read(AccessObjectKind::Role, namespace, name).await {
        Ok(labels) => labels,
        Err(error) => {
            return AccessOutcome::Failed {
                error: error.to_string(),
            }
        }
    };
    let binding_labels = match store
        .read(AccessObjectKind::RoleBinding, namespace, name)
        .await
    {
        Ok(labels) => labels,
        Err(error) => {
            return AccessOutcome::Failed {
                error: error.to_string(),
            }
        }
    };

    let mut adopting = false;
    for labels in [&role_labels, &binding_labels].into_iter().flatten() {
        match ownership(labels, owner) {
            Ownership::Foreign { owner } => {
                return AccessOutcome::OwnerConflict {
                    foreign_owner: owner,
                }
            }
            Ownership::Unowned => adopting = true,
            Ownership::Owned => {}
        }
    }
    if adopting && !adopt_unowned {
        return AccessOutcome::AdoptionRefusedUnowned;
    }

    for (kind, object) in [
        (AccessObjectKind::Role, role),
        (AccessObjectKind::RoleBinding, binding),
    ] {
        if let Err(error) = store.apply(kind, namespace, name, object).await {
            return AccessOutcome::Failed {
                error: error.to_string(),
            };
        }
    }
    if adopting {
        AccessOutcome::Adopted
    } else {
        AccessOutcome::Applied
    }
}

/// Prune one stale candidate that this owner created; keep foreign and unowned objects.
async fn prune_candidate(
    store: &dyn AccessStore,
    owner: &str,
    namespace: &str,
    name: &str,
) -> Option<AccessOutcome> {
    let role = store.read(AccessObjectKind::Role, namespace, name).await;
    let binding = store
        .read(AccessObjectKind::RoleBinding, namespace, name)
        .await;
    let (role, binding) = match (role, binding) {
        (Ok(role), Ok(binding)) => (role, binding),
        (Err(error), _) | (_, Err(error)) => {
            return Some(AccessOutcome::Failed {
                error: error.to_string(),
            })
        }
    };
    if role.is_none() && binding.is_none() {
        return None;
    }

    let all_owned = [&role, &binding]
        .into_iter()
        .flatten()
        .all(|labels| ownership(labels, owner) == Ownership::Owned);
    if !all_owned {
        return Some(AccessOutcome::ForeignRetained);
    }

    for (kind, exists) in [
        (AccessObjectKind::Role, role.is_some()),
        (AccessObjectKind::RoleBinding, binding.is_some()),
    ] {
        if exists {
            if let Err(error) = store.delete(kind, namespace, name).await {
                return Some(AccessOutcome::Failed {
                    error: error.to_string(),
                });
            }
        }
    }
    Some(AccessOutcome::Pruned)
}

/// Apply the declared grants, adopt or refuse unowned ones, and prune stale
/// candidates that this owner created.
///
/// An invalid policy makes no store call. A failure in one grant never stops
/// the others. Only `Role` and `RoleBinding` objects are ever written.
pub async fn reconcile_access(
    store: &dyn AccessStore,
    owner: &str,
    grants: &[AccessGrant],
    prune_candidates: &[(String, String)],
    adopt_unowned: bool,
) -> AccessReport {
    if validate_policy(grants).is_err() {
        return report(BTreeMap::new(), grants, &[]);
    }

    let mut sorted = grants.to_vec();
    sorted.sort_by(|a, b| (&a.namespace, &a.name).cmp(&(&b.namespace, &b.name)));
    let rendered = render_access_grants(&sorted);

    let mut outcomes = BTreeMap::new();
    for (grant, objects) in sorted.iter().zip(rendered.chunks(2)) {
        let mut role = objects[0].clone();
        let mut binding = objects[1].clone();
        role["metadata"]["labels"][OWNER_IDENTITY_LABEL] = json!(owner);
        binding["metadata"]["labels"][OWNER_IDENTITY_LABEL] = json!(owner);
        let outcome = apply_grant(
            store,
            owner,
            &grant.namespace,
            &grant.name,
            role,
            binding,
            adopt_unowned,
        )
        .await;
        outcomes.insert((grant.namespace.clone(), grant.name.clone()), outcome);
    }

    for (namespace, name) in prune_candidates {
        let declared = grants
            .iter()
            .any(|grant| grant.namespace == *namespace && grant.name == *name);
        if declared {
            continue;
        }
        if let Some(outcome) = prune_candidate(store, owner, namespace, name).await {
            outcomes.insert((namespace.clone(), name.clone()), outcome);
        }
    }

    let grant_outcomes: Vec<GrantOutcome> = grants
        .iter()
        .map(|grant| {
            let applied = outcomes
                .get(&(grant.namespace.clone(), grant.name.clone()))
                .is_some_and(AccessOutcome::is_applied);
            GrantOutcome::new(grant.namespace.clone(), grant.name.clone(), applied)
        })
        .collect();
    report(outcomes, grants, &grant_outcomes)
}
