//! Data retention of a service instance on deletion (#517).
//!
//! A service that returns a [`RetentionPlan`] gets a second finalizer. On
//! deletion the controller stops the named workloads and, with
//! [`DataRetention::Retain`], deletes no PersistentVolumeClaim.

use std::collections::BTreeMap;

/// What happens to an instance's data when the instance is deleted.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "PascalCase")]
pub enum DataRetention {
    /// Keep the data: stop the workloads and delete no PVC.
    #[default]
    Retain,
    /// Delete the instance's own PVCs with the instance (see #519).
    Delete,
}

/// The label that marks an instance's own resources, stamped by the
/// rendered PVC templates.
pub const INSTANCE_UID_LABEL: &str = "service-k8s.axiom.dev/instance-uid";

/// The retention plan of one instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionPlan {
    /// The policy to apply on deletion.
    pub policy: DataRetention,
    /// The workloads to stop, as `(kind, name)`; kind is `StatefulSet` or `Deployment`.
    pub workloads: Vec<(&'static str, String)>,
    /// The labels that select this instance's PVCs.
    pub pvc_labels: BTreeMap<String, String>,
}

impl RetentionPlan {
    /// The PVC label selector as `k=v,k=v`, in map order.
    pub fn selector(&self) -> String {
        self.pvc_labels
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(",")
    }
}
