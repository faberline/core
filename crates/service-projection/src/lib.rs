//! Shared typed projection runtime.
//!
//! The runtime owns checkpoints, catch-up, rebuild, publication, and flush.
//! Products keep typed handles and define their record and projection logic.

mod application;
mod domain;
mod infrastructure;

pub use application::{ProjectionHandle, ProjectionRegistry, ProjectionRuntimeConfig};
pub use domain::{
    Projection, ProjectionCheckpoint, ProjectionDescriptor, ProjectionLag, ProjectionReadSession,
    ProjectionRecord, ProjectionSource, RebuildComparison,
};
pub use infrastructure::{ProjectionStateEnvelope, PROJECTION_STATE_FORMAT_VERSION};
