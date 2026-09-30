//! Projection values and the ports a product implements: descriptors,
//! checkpoints, records, sources, lag, rebuild comparison and the
//! projection error; and the state store port the runtime saves through.

mod checkpoint;
mod digest;
mod lag;
mod projection;
mod projection_error;
mod projection_state_store;
mod rebuild;
mod source;

pub(crate) use checkpoint::checkpoint;
pub use checkpoint::ProjectionCheckpoint;
pub(crate) use digest::sha256;
pub use lag::ProjectionLag;
pub(crate) use projection::validate_descriptor;
pub use projection::{Projection, ProjectionDescriptor};
pub use projection_error::ProjectionError;
pub(crate) use projection_state_store::ProjectionStateStore;
pub use rebuild::RebuildComparison;
pub use source::{ProjectionReadSession, ProjectionRecord, ProjectionSource};
