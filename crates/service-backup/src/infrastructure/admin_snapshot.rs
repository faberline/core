//! Shared HTTP transport for a service's standard admin snapshot endpoint.

mod lenient_fetch;
#[cfg(test)]
mod tests;
mod transport;

pub use lenient_fetch::fetch_admin_snapshot;
pub use transport::{
    AdminSnapshotDiagnostic, AdminSnapshotOperation, AdminSnapshotRequest,
    AdminSnapshotRequestError, AdminSnapshotTransport, AdminSnapshotTransportConfig,
    AdminSnapshotTransportError,
};
