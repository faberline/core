//! The [`ManagedService`] trait a service implements + the shared CRD fragments.

pub use crate::application::condition::now_rfc3339;
#[cfg(feature = "controller")]
pub use crate::application::operator::managed_service::{
    ClusterScopedChild, ManagedService, PruneTarget, ReadinessTarget, ReadyFacts, ReconcilePlan,
};
pub use crate::domain::condition::{project, Condition, ConditionFact, ConditionStatus};
pub use crate::interfaces::cluster_spec::{ClusterSpec, ResourceSpec};
