//! Use cases: one backup run, and the admin-snapshot backup built on it.

#[cfg(feature = "http-client")]
mod admin_snapshot_backup;
mod runner;

#[cfg(feature = "http-client")]
pub(crate) use admin_snapshot_backup::run_snapshot_backup;
pub use runner::{run_backup_once, BackupObject, BackupRunResult};
