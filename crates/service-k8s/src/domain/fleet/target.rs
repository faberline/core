//! Fleet target: the cluster and namespace that one Fleet entry applies to.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Names the cluster and namespace of one Fleet entry.
///
/// `cluster: None` means the control-plane cluster.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct FleetTarget {
    /// Cluster name. `None` means the control-plane cluster.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster: Option<String>,
    /// Namespace in that cluster.
    pub namespace: String,
}

impl FleetTarget {
    /// Create a target in a cluster, or in the control-plane cluster when `cluster` is `None`.
    pub fn new(cluster: Option<String>, namespace: impl Into<String>) -> Self {
        Self {
            cluster,
            namespace: namespace.into(),
        }
    }

    /// Create a target in the control-plane cluster.
    pub fn local(namespace: impl Into<String>) -> Self {
        Self::new(None, namespace)
    }

    /// Return the cluster name. `"local"` when the cluster is `None` or `Some("local")`.
    pub fn cluster_name(&self) -> &str {
        match self.cluster.as_deref() {
            None | Some("local") => "local",
            Some(name) => name,
        }
    }
}
