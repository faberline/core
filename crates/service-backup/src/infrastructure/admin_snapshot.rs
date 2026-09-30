//! Shared HTTP transport for a service's standard admin snapshot endpoint,
//! and that endpoint as the domain's snapshot source.

mod snapshot_endpoint;
#[cfg(test)]
mod tests;
mod transport;

pub(crate) use snapshot_endpoint::AdminSnapshotEndpoint;
pub use transport::{
    AdminSnapshotDiagnostic, AdminSnapshotOperation, AdminSnapshotRequest,
    AdminSnapshotRequestError, AdminSnapshotTransport, AdminSnapshotTransportConfig,
    AdminSnapshotTransportError,
};
