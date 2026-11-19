//! The composition root: wiring that may use every layer.

#[cfg(feature = "http-client")]
mod admin_snapshot_backup;
#[cfg(feature = "http-client")]
mod backup_verb;
#[cfg(feature = "http-client")]
mod projected_bearer;
mod sink_from_destination;

#[cfg(feature = "http-client")]
pub use admin_snapshot_backup::run_admin_snapshot_backup;
#[cfg(feature = "http-client")]
pub use backup_verb::{
    render_backup_output, run_backup_verb, run_backup_verb_payload, BackupVerbParams, NextMarker,
};
pub use sink_from_destination::sink_from_destination;
