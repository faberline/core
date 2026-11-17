//! Pure planner that migrates a single StatefulSet to the shard-0 StatefulSet.
//!
//! A StatefulSet selector and its `volumeClaimTemplates` are immutable. The
//! planner compares the live StatefulSet with the desired shard-0 one and says
//! whether it can be updated in place, must be recreated with orphaned pods
//! (so the PVCs stay), or must be refused. It never plans a PVC deletion.
//! This module does no I/O.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The parts of a StatefulSet that decide whether it can be updated in place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatefulSetShape {
    /// `metadata.name`.
    pub name: String,
    /// `spec.selector.matchLabels`.
    pub selector: BTreeMap<String, String>,
    /// `spec.volumeClaimTemplates[*].metadata.name`, in order.
    pub claim_templates: Vec<String>,
}

/// A required field is missing from a StatefulSet JSON object.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShardMigrationError {
    /// The named field is missing or has the wrong type.
    #[error("statefulset is missing `{0}`")]
    Missing(&'static str),
}

/// What the controller must do to reach the desired shard-0 StatefulSet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardMigrationPlan {
    /// The selector already matches: apply the desired object in place.
    NoChange,
    /// Delete the StatefulSet with `propagationPolicy=Orphan` (pods and PVCs
    /// stay), then create the desired one.
    RecreateOrphan,
    /// Recreating could lose data or is not supported. The text says why.
    Refuse(String),
}

impl StatefulSetShape {
    /// Parse a StatefulSet JSON object.
    ///
    /// `metadata.name` and `spec.selector.matchLabels` are required.
    /// `spec.volumeClaimTemplates` is optional; each template needs
    /// `metadata.name`.
    ///
    /// # Errors
    ///
    /// Returns [`ShardMigrationError::Missing`] for a missing required field.
    pub fn from_value(sts: &Value) -> Result<Self, ShardMigrationError> {
        let name = sts
            .pointer("/metadata/name")
            .and_then(Value::as_str)
            .ok_or(ShardMigrationError::Missing("metadata.name"))?
            .to_string();

        let selector = sts
            .pointer("/spec/selector/matchLabels")
            .and_then(Value::as_object)
            .ok_or(ShardMigrationError::Missing("spec.selector.matchLabels"))?
            .iter()
            .map(|(key, value)| {
                value
                    .as_str()
                    .map(|v| (key.clone(), v.to_string()))
                    .ok_or(ShardMigrationError::Missing("spec.selector.matchLabels"))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        let claim_templates = match sts
            .pointer("/spec/volumeClaimTemplates")
            .and_then(Value::as_array)
        {
            None => Vec::new(),
            Some(templates) => templates
                .iter()
                .map(|t| {
                    t.pointer("/metadata/name")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .ok_or(ShardMigrationError::Missing(
                            "volumeClaimTemplates[].metadata.name",
                        ))
                })
                .collect::<Result<Vec<_>, _>>()?,
        };

        Ok(Self {
            name,
            selector,
            claim_templates,
        })
    }

    /// The PVC names the StatefulSet controller creates for `replicas` pods:
    /// for each claim template in order, for each ordinal `r` in `0..replicas`,
    /// `<claim>-<statefulset name>-<r>`.
    pub fn pvc_names(&self, replicas: u32) -> Vec<String> {
        self.claim_templates
            .iter()
            .flat_map(|claim| (0..replicas).map(move |r| format!("{claim}-{}-{r}", self.name)))
            .collect()
    }
}

/// Plan the move from the observed StatefulSet to the desired shard-0 one.
///
/// `observed_shard_count` is the `shard_count` of the existing instance.
/// Rules, checked in order; the first that applies wins:
/// 1. no `shard` selector key and `observed_shard_count > 1` -> `Refuse`;
/// 2. the names differ -> `Refuse`;
/// 3. the claim templates differ -> `Refuse`;
/// 4. the selectors are equal -> `NoChange`;
/// 5. otherwise -> `RecreateOrphan`.
pub fn plan_shard_migration(
    observed: &StatefulSetShape,
    desired: &StatefulSetShape,
    observed_shard_count: u32,
) -> ShardMigrationPlan {
    if !observed.selector.contains_key("shard") && observed_shard_count > 1 {
        return ShardMigrationPlan::Refuse("multi-shard ordinal layout: not migrated".to_string());
    }
    if observed.name != desired.name {
        return ShardMigrationPlan::Refuse(format!(
            "statefulset name differs: {} -> {}",
            observed.name, desired.name
        ));
    }
    if observed.claim_templates != desired.claim_templates {
        return ShardMigrationPlan::Refuse(format!(
            "volumeClaimTemplates differ: {:?} -> {:?}",
            observed.claim_templates, desired.claim_templates
        ));
    }
    if observed.selector == desired.selector {
        return ShardMigrationPlan::NoChange;
    }
    ShardMigrationPlan::RecreateOrphan
}

#[cfg(test)]
mod tests;
