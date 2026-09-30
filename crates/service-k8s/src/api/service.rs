//! The [`ManagedService`] trait a service implements + the shared CRD fragments.

pub use crate::domain::condition::{
    now_rfc3339, project, Condition, ConditionFact, ConditionStatus,
};
pub use crate::interfaces::cluster_spec::{ClusterSpec, ResourceSpec};
#[cfg(feature = "controller")]
pub use crate::interfaces::operator::managed_service::{
    ClusterScopedChild, ManagedService, PruneTarget, ReadinessTarget, ReadyFacts, ReconcilePlan,
};
