//! Domain layer.

pub(crate) mod backup_job;
pub(crate) mod capacity;
#[cfg(feature = "certificate")]
pub(crate) mod certificate;
pub(crate) mod condition;
#[cfg(feature = "controller")]
pub mod fleet;
#[cfg(feature = "controller")]
pub(crate) mod leadership;
pub(crate) mod lifecycle;
pub mod trust_bundle;
pub(crate) mod rollout;
