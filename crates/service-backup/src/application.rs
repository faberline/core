//! Use cases: one backup run, the admin-snapshot backup built on it, and
//! the query for which destination schemes a run in this build can write.

#[cfg(feature = "http-client")]
mod admin_snapshot_backup;
mod destination_schemes;
mod runner;

#[cfg(feature = "http-client")]
pub(crate) use admin_snapshot_backup::run_snapshot_backup;
pub(crate) use destination_schemes::{destination_schemes, SinkSupport};
pub use runner::{run_backup_once, BackupObject, BackupRunResult};
