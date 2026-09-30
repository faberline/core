//! The on-disk projection state store: the file adapter of the domain
//! state store port, the versioned state envelope, atomic persist and
//! checked restore, quarantine of invalid snapshots, and private file modes.

mod file_mode;
mod file_state_store;
mod quarantine;
mod state_store;

pub(crate) use file_state_store::FileProjectionStateStore;
pub use state_store::{ProjectionStateEnvelope, PROJECTION_STATE_FORMAT_VERSION};
