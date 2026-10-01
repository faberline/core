//! `service-backup` — shared backup contract for axiom services.
//!
//! The data plane owns snapshot consistency: each service state machine produces
//! bytes at a concrete applied index, and `raft-runtime` handles snapshot install
//! plus log compaction. This crate owns the cross-service backup shape around
//! those bytes: destination/policy schema, sink trait, local sink, and a small
//! runner primitive.
//!
//! Operator code should render/manage a backup runner from the policy. The
//! runner calls the service's admin backup endpoint or CLI, then writes the
//! returned bytes through a [`BackupSink`]. Local and GCS are always available;
//! S3 is feature-gated. GCS uses workload identity in production and Vat's
//! `STORAGE_EMULATOR_HOST` locally. Bootstrap/restore reads exact object URIs
//! through [`fetch_backup_object`]. The optional `http-client` feature adds the
//! standard authenticated admin-snapshot transport used by service backup CLIs.

mod application;
mod compat;
mod domain;
mod infrastructure;
mod interfaces;

#[cfg(feature = "http-client")]
pub use application::run_admin_snapshot_backup;
pub use application::{run_backup_once, BackupObject, BackupRunResult};
pub use compat::llm;
pub use domain::{
    BackupDestination, BackupPolicy, RetentionPolicy, ScheduledBackupPolicy, SchemeInfo,
    SUPPORTED_SCHEMES,
};
#[cfg(feature = "http-client")]
pub use infrastructure::{
    fetch_admin_snapshot, AdminSnapshotDiagnostic, AdminSnapshotOperation, AdminSnapshotRequest,
    AdminSnapshotRequestError, AdminSnapshotTransport, AdminSnapshotTransportConfig,
    AdminSnapshotTransportError,
};
pub use infrastructure::{
    fetch_backup_object, sink_from_destination, BackupSink, GcsSink, LocalFsSink,
    UnsupportedCloudSink,
};
