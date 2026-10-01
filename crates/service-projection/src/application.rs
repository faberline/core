//! The projection runtime: typed handles that restore, catch up, rebuild
//! and flush one projection, the registry over them, and runtime
//! configuration.

mod config;
mod handle;
mod registry;

pub use config::ProjectionRuntimeConfig;
pub use handle::ProjectionHandle;
pub use registry::ProjectionRegistry;
