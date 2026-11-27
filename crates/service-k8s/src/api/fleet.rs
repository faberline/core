//! Generic Fleet kind: one spec shape that serves any child kind.
//!
//! An app declares its Fleet kind by naming its child spec type. The engine
//! reads the instances as opaque JSON through [`FleetSpec::entries`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api::crd::normalize_unsigned_integer_formats;
use crate::domain::condition::{self, Condition, ConditionFact, ConditionStatus};
use crate::domain::fleet::{
    aggregate_fleet_condition, AdoptionPolicy, FleetEntry, FleetEntryStatus, FleetTarget,
    PrunePolicy,
};

/// One instance of a Fleet: where it runs, its optional name and its spec.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FleetInstance<S> {
    /// Cluster and namespace of the instance.
    pub target: FleetTarget,
    /// Object name. `None` uses the default name of the caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Child spec of this instance.
    pub spec: S,
}

/// How many instances a rollout changes at the same time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FleetRollout {
    /// Largest number of entries that change at once.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
}

fn default_max_concurrent() -> u32 {
    1
}

impl Default for FleetRollout {
    fn default() -> Self {
        Self { max_concurrent: 1 }
    }
}

/// Desired state of a Fleet of child objects of one kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FleetSpec<S> {
    /// Desired instances, in declared order.
    pub instances: Vec<FleetInstance<S>>,
    /// Rollout limits.
    #[serde(default)]
    pub rollout: FleetRollout,
    /// What happens to owned objects that are no longer desired.
    #[serde(default)]
    pub prune: PrunePolicy,
    /// What happens to unowned objects that match a desired instance.
    #[serde(default)]
    pub adoption: AdoptionPolicy,
}

impl<S: Serialize> FleetSpec<S> {
    /// Convert the instances into opaque entries for the Fleet engine.
    ///
    /// Each entry keeps the declared order and carries the spec as JSON.
    pub fn entries(&self) -> Result<Vec<FleetEntry>, serde_json::Error> {
        self.instances
            .iter()
            .map(|instance| {
                Ok(FleetEntry {
                    target: instance.target.clone(),
                    name: instance.name.clone(),
                    spec: serde_json::to_value(&instance.spec)?,
                })
            })
            .collect()
    }
}

/// Observed status of a Fleet: one entry status per instance and one aggregate condition.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FleetStatus {
    /// Status of each entry.
    #[serde(default)]
    pub entries: Vec<FleetEntryStatus>,
    /// Aggregate conditions of the Fleet.
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

impl FleetStatus {
    /// Build the status from the entry statuses and the prior conditions.
    ///
    /// The result holds one `Ready` condition. It is `True` when every entry is
    /// ready, and `False` otherwise. The transition time carries over from `prior`.
    pub fn project(
        prior: &[Condition],
        entries: Vec<FleetEntryStatus>,
        observed_generation: i64,
        now: &str,
    ) -> FleetStatus {
        let aggregate = aggregate_fleet_condition(&entries);
        let (status, reason) = if aggregate.ready {
            (ConditionStatus::True, "AllEntriesReady")
        } else {
            (ConditionStatus::False, "EntriesNotReady")
        };
        let fact = ConditionFact::new("Ready", status, reason, aggregate.message);
        let conditions = condition::project(prior, vec![fact], observed_generation, now);
        FleetStatus {
            entries,
            conditions,
        }
    }
}

/// Structural CRD schema of a Fleet of `S` instances.
///
/// `child_kind` names the child kind in the schema description. The schema has
/// no `x-kubernetes-preserve-unknown-fields` at the root.
pub fn fleet_crd_schema_hint<S: JsonSchema>(child_kind: &str) -> serde_json::Value {
    let settings = schemars::generate::SchemaSettings::openapi3().with(|s| {
        s.inline_subschemas = true;
    });
    let generator = settings.into_generator();
    let root = generator.into_root_schema_for::<FleetSpec<S>>();
    let mut spec = serde_json::to_value(&root).expect("schema serializes");
    normalize_unsigned_integer_formats(&mut spec);
    if let serde_json::Value::Object(map) = &mut spec {
        map.remove("$schema");
        map.remove("title");
        map.remove("definitions");
        map.remove("$defs");
        map.insert(
            "description".to_string(),
            json!(format!("Fleet of {child_kind} instances")),
        );
    }
    spec
}
