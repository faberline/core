//! The composition root: wiring that may use every layer.
//!
//! Each function here is a public entry point: it builds the adapters the use
//! case needs and hands them over.

#[cfg(feature = "k8s")]
pub(crate) mod connect;
#[cfg(feature = "online")]
pub(crate) mod issue;
#[cfg(feature = "online")]
pub(crate) mod upgrade;
