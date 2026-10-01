//! A service's `GET /admin/backup` endpoint, read through the strict
//! transport, as the domain's [`SnapshotSource`].

use std::future::Future;
use std::pin::Pin;

use super::AdminSnapshotTransport;
use crate::domain::{SnapshotSource, SnapshotSourceError};

/// One service's admin snapshot endpoint.
///
/// Each fetch is one [`AdminSnapshotTransport::fetch_exact`] call: production
/// timeouts, no redirects, exactly `200 OK`, and a redacted error.
pub(crate) struct AdminSnapshotEndpoint {
    transport: AdminSnapshotTransport,
    base_url: String,
    bearer: Option<String>,
}

impl AdminSnapshotEndpoint {
    pub(crate) fn new(
        transport: AdminSnapshotTransport,
        base_url: impl Into<String>,
        bearer: Option<&str>,
    ) -> Self {
        Self {
            transport,
            base_url: base_url.into(),
            bearer: bearer.map(str::to_owned),
        }
    }
}

impl SnapshotSource for AdminSnapshotEndpoint {
    fn fetch_snapshot(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, SnapshotSourceError>> + Send + '_>> {
        Box::pin(async move {
            self.transport
                .fetch_exact(&self.base_url, self.bearer.as_deref())
                .await
                .map_err(SnapshotSourceError::other)
        })
    }
}
