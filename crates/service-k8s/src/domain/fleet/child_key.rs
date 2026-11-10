//! Kubernetes child object identifier for Fleet-managed resources.

use serde::{Deserialize, Serialize};

/// Identifies one Kubernetes child object managed or targeted by a Fleet.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ChildKey {
    pub api_version: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub name: String,
}

impl ChildKey {
    /// Create a new child key.
    pub fn new(
        api_version: impl Into<String>,
        kind: impl Into<String>,
        namespace: Option<impl Into<String>>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            api_version: api_version.into(),
            kind: kind.into(),
            namespace: namespace.map(Into::into),
            name: name.into(),
        }
    }

    /// Create a namespaced child key.
    pub fn namespaced(
        api_version: impl Into<String>,
        kind: impl Into<String>,
        namespace: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            api_version: api_version.into(),
            kind: kind.into(),
            namespace: Some(namespace.into()),
            name: name.into(),
        }
    }

    /// Create a cluster-scoped child key with no namespace.
    pub fn cluster_scoped(
        api_version: impl Into<String>,
        kind: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            api_version: api_version.into(),
            kind: kind.into(),
            namespace: None,
            name: name.into(),
        }
    }
}

impl std::fmt::Display for ChildKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ns) = &self.namespace {
            write!(f, "{}/{}/{}/{}", self.api_version, self.kind, ns, self.name)
        } else {
            write!(f, "{}/{}/{}", self.api_version, self.kind, self.name)
        }
    }
}
