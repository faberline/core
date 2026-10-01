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
pub(crate) use admin_snapshot::AdminSnapshotEndpoint;
#[cfg(feature = "http-client")]
pub use admin_snapshot::{
    AdminSnapshotDiagnostic, AdminSnapshotOperation, AdminSnapshotRequest,
    AdminSnapshotRequestError, AdminSnapshotTransport, AdminSnapshotTransportConfig,
    AdminSnapshotTransportError,
};
pub use gcs::GcsSink;
#[cfg(feature = "s3")]
pub(crate) use s3::S3Sink;
pub use sink::{LocalFsSink, UnsupportedCloudSink};
pub use source::fetch_backup_object;
