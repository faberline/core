//! Projection values and the ports a product implements: descriptors,
//! checkpoints, records, sources, lag and rebuild comparison.

mod checkpoint;
mod digest;
mod lag;
mod projection;
mod rebuild;
mod source;

pub(crate) use checkpoint::checkpoint;
pub use checkpoint::ProjectionCheckpoint;
pub(crate) use digest::sha256;
pub use lag::ProjectionLag;
pub(crate) use projection::validate_descriptor;
pub use projection::{Projection, ProjectionDescriptor};
pub use rebuild::RebuildComparison;
pub use source::{ProjectionReadSession, ProjectionRecord, ProjectionSource};
