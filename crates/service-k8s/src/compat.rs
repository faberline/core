//! Old public module paths, kept as re-exports.

#[cfg(feature = "certificate")]
pub mod certificate;
#[cfg(feature = "controller")]
pub mod controller;
pub mod crd;
#[cfg(feature = "controller")]
pub mod lease;
pub mod lifecycle;
#[cfg(feature = "controller")]
pub mod llm;
#[cfg(feature = "controller")]
pub mod metrics;
pub mod render;
#[cfg(feature = "controller")]
pub mod resize;
pub mod service;
pub mod stateful;
