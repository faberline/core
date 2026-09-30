//! The composition root: wiring that may use every layer.

#[cfg(feature = "http-client")]
mod admin_snapshot_backup;
mod sink_from_destination;

#[cfg(feature = "http-client")]
pub use admin_snapshot_backup::run_admin_snapshot_backup;
pub use sink_from_destination::sink_from_destination;
