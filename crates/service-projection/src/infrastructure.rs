//! The on-disk projection state store: the versioned state envelope,
//! atomic persist and checked restore, quarantine of invalid snapshots,
//! and private file modes.

mod file_mode;
mod quarantine;
mod state_store;

pub(crate) use file_mode::set_directory_mode;
pub(crate) use quarantine::quarantine_invalid_snapshot;
pub(crate) use state_store::{persist, restore_snapshot};
pub use state_store::{ProjectionStateEnvelope, PROJECTION_STATE_FORMAT_VERSION};
