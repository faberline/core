//! Adapters: the local, GCS and S3 sinks, exact object fetch, and the admin
//! snapshot HTTP transport.

#[cfg(feature = "http-client")]
mod admin_snapshot;
mod gcs;
#[cfg(feature = "s3")]
mod s3;
mod sink;
mod source;

#[cfg(feature = "http-client")]
pub use admin_snapshot::{
    fetch_admin_snapshot, AdminSnapshotDiagnostic, AdminSnapshotOperation, AdminSnapshotRequest,
    AdminSnapshotRequestError, AdminSnapshotTransport, AdminSnapshotTransportConfig,
    AdminSnapshotTransportError,
};
pub use gcs::GcsSink;
pub use sink::{sink_from_destination, BackupSink, LocalFsSink, UnsupportedCloudSink};
pub use source::fetch_backup_object;
