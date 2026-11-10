//! Pure reconciliation planning for Fleet child resources.

use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};

use super::{AdoptionPolicy, ChildKey, FleetPlanError, Ownership, PrunePolicy};

/// One desired child object targeted by a Fleet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesiredChild {
    pub key: ChildKey,
    pub desired_hash: String,
}

impl DesiredChild {
    pub fn new(key: ChildKey, desired_hash: impl Into<String>) -> Self {
        Self {
            key,
            desired_hash: desired_hash.into(),
        }
    }
}

/// One observed child object currently present in the cluster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedChild {
    pub key: ChildKey,
    pub ownership: Ownership,
    pub observed_hash: Option<String>,
}

impl ObservedChild {
    pub fn new(
        key: ChildKey,
        ownership: Ownership,
        observed_hash: Option<impl Into<String>>,
    ) -> Self {
        Self {
            key,
            ownership,
            observed_hash: observed_hash.map(Into::into),
        }
    }

    pub fn owned(key: ChildKey, observed_hash: impl Into<String>) -> Self {
        Self {
            key,
            ownership: Ownership::Owned,
            observed_hash: Some(observed_hash.into()),
        }
    }

    pub fn unowned(key: ChildKey) -> Self {
        Self {
            key,
            ownership: Ownership::Unowned,
            observed_hash: None,
        }
    }

    pub fn foreign(key: ChildKey, owner: impl Into<String>) -> Self {
        Self {
            key,
            ownership: Ownership::Foreign {
                owner: owner.into(),
            },
            observed_hash: None,
        }
    }
}

/// Planned action for one child object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FleetAction {
    /// Object does not exist observed: create it with the desired hash.
    Create { key: ChildKey, desired_hash: String },
    /// Object exists and is owned, but desired hash differs from observed: update it.
    Update { key: ChildKey, desired_hash: String },
    /// Object exists, is owned, and desired hash matches observed: no change needed.
    Unchanged { key: ChildKey },
    /// Object exists, is unowned, and policy is [`AdoptionPolicy::Adopt`]: adopt it.
    Adopt { key: ChildKey, desired_hash: String },
    /// Object is foreign-owned or unowned under [`AdoptionPolicy::Refuse`]: skip it.
    SkipForeign { key: ChildKey },
    /// Object is owned but absent from desired, and policy is [`PrunePolicy::Delete`]: delete it.
    Prune { key: ChildKey },
    /// Object is owned but absent from desired, and policy is [`PrunePolicy::Retain`]: report as orphan.
    Orphan { key: ChildKey },
}

impl FleetAction {
    /// The target child key for this action.
    pub fn key(&self) -> &ChildKey {
        match self {
            Self::Create { key, .. }
            | Self::Update { key, .. }
            | Self::Unchanged { key }
            | Self::Adopt { key, .. }
            | Self::SkipForeign { key }
            | Self::Prune { key }
            | Self::Orphan { key } => key,
        }
    }
}

/// The planned set of actions for a Fleet reconcile pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetPlan {
    pub actions: Vec<FleetAction>,
}

impl FleetPlan {
    pub fn new(actions: Vec<FleetAction>) -> Self {
        Self { actions }
    }

    pub fn actions(&self) -> &[FleetAction] {
        &self.actions
    }
}

/// Plan Fleet child reconciliations given desired and observed states and adoption/prune policies.
///
/// Returns [`FleetPlanError::DuplicateTarget`] if two desired entries share the same [`ChildKey`],
/// before producing any action.
///
/// Actions in [`FleetPlan`] are strictly ordered by [`ChildKey`] (api_version, kind, namespace, name).
pub fn plan_fleet(
    desired: impl IntoIterator<Item = impl Borrow<DesiredChild>>,
    observed: impl IntoIterator<Item = impl Borrow<ObservedChild>>,
    adoption: AdoptionPolicy,
    prune: PrunePolicy,
) -> Result<FleetPlan, FleetPlanError> {
    let mut desired_map: BTreeMap<ChildKey, String> = BTreeMap::new();
    for item in desired {
        let d = item.borrow();
        if desired_map.contains_key(&d.key) {
            return Err(FleetPlanError::DuplicateTarget(d.key.clone()));
        }
        desired_map.insert(d.key.clone(), d.desired_hash.clone());
    }

    let mut observed_map: BTreeMap<ChildKey, (Ownership, Option<String>)> = BTreeMap::new();
    for item in observed {
        let o = item.borrow();
        observed_map.insert(
            o.key.clone(),
            (o.ownership.clone(), o.observed_hash.clone()),
        );
    }

    let mut all_keys: BTreeSet<ChildKey> = BTreeSet::new();
    all_keys.extend(desired_map.keys().cloned());
    all_keys.extend(observed_map.keys().cloned());

    let mut actions = Vec::with_capacity(all_keys.len());

    for key in all_keys {
        let desired_hash_opt = desired_map.get(&key);
        let observed_info = observed_map.get(&key);

        match (desired_hash_opt, observed_info) {
            (Some(desired_hash), None) => {
                actions.push(FleetAction::Create {
                    key,
                    desired_hash: desired_hash.clone(),
                });
            }
            (Some(desired_hash), Some((ownership, observed_hash))) => match ownership {
                Ownership::Owned => {
                    if observed_hash.as_deref() == Some(desired_hash.as_str()) {
                        actions.push(FleetAction::Unchanged { key });
                    } else {
                        actions.push(FleetAction::Update {
                            key,
                            desired_hash: desired_hash.clone(),
                        });
                    }
                }
                Ownership::Unowned => match adoption {
                    AdoptionPolicy::Adopt => {
                        actions.push(FleetAction::Adopt {
                            key,
                            desired_hash: desired_hash.clone(),
                        });
                    }
                    AdoptionPolicy::Refuse => {
                        actions.push(FleetAction::SkipForeign { key });
                    }
                },
                Ownership::Foreign { .. } => {
                    actions.push(FleetAction::SkipForeign { key });
                }
            },
            (None, Some((ownership, _observed_hash))) => match ownership {
                Ownership::Owned => match prune {
                    PrunePolicy::Delete => {
                        actions.push(FleetAction::Prune { key });
                    }
                    PrunePolicy::Retain => {
                        actions.push(FleetAction::Orphan { key });
                    }
                },
                Ownership::Foreign { .. } | Ownership::Unowned => {
                    actions.push(FleetAction::SkipForeign { key });
                }
            },
            (None, None) => {}
        }
    }

    Ok(FleetPlan { actions })
}
