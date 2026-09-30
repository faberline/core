//! Domain layer.

pub(crate) mod capacity;
#[cfg(feature = "certificate")]
pub(crate) mod certificate;
pub(crate) mod condition;
#[cfg(feature = "controller")]
pub(crate) mod leadership;
pub(crate) mod lifecycle;
