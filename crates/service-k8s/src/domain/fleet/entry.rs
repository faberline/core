//! Fleet entry: one desired instance in a Fleet, with an opaque spec.

use serde::{Deserialize, Serialize};

use super::{ChildKey, FleetPlanError, FleetTarget};

/// One desired instance of a Fleet. The spec is opaque data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetEntry {
    /// Where the instance runs.
    pub target: FleetTarget,
    /// Object name. `None` uses the default name of the caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Instance spec, kept as opaque JSON.
    pub spec: serde_json::Value,
}

impl FleetEntry {
    /// Return the child key of this entry. `default_name` is used when `name` is `None`.
    pub fn child_key(&self, api_version: &str, kind: &str, default_name: &str) -> ChildKey {
        ChildKey::namespaced(
            api_version,
            kind,
            &self.target.namespace,
            self.name.as_deref().unwrap_or(default_name),
        )
    }
}

/// Reject two entries that resolve to the same cluster and child key.
///
/// Returns [`FleetPlanError::DuplicateTarget`] with the key of the second entry.
pub fn reject_duplicate_targets(
    entries: &[FleetEntry],
    api_version: &str,
    kind: &str,
    default_name: &str,
) -> Result<(), FleetPlanError> {
    let mut seen: Vec<(&str, ChildKey)> = Vec::with_capacity(entries.len());
    for entry in entries {
        let cluster = entry.target.cluster_name();
        let key = entry.child_key(api_version, kind, default_name);
        if seen.iter().any(|(c, k)| *c == cluster && *k == key) {
            return Err(FleetPlanError::DuplicateTarget(key));
        }
        seen.push((cluster, key));
    }
    Ok(())
}
