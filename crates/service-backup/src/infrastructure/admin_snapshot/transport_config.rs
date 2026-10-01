//! The limits one [`AdminSnapshotTransport`](super::AdminSnapshotTransport)
//! enforces on every request.

use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const BACKUP_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Shared transport limits. Products may lower these values but do not need to
/// own request, redirect, retry, idle-read, or diagnostic-body control flow.
///
/// Start from [`Default`] (10 s connect, 2 h operation, 30 s idle read, 8 KiB
/// diagnostic body) and change one limit with a `with_*` builder:
///
/// ```
/// use std::time::Duration;
/// use service_backup::AdminSnapshotTransportConfig;
///
/// let config = AdminSnapshotTransportConfig::default()
///     .with_operation_timeout(Duration::from_secs(60))
///     .with_max_diagnostic_bytes(1024);
/// assert_eq!(config.operation_timeout(), Duration::from_secs(60));
/// assert_eq!(config.max_diagnostic_bytes(), 1024);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct AdminSnapshotTransportConfig {
    connect_timeout: Duration,
    operation_timeout: Duration,
    response_idle_timeout: Duration,
    max_diagnostic_bytes: usize,
}

impl Default for AdminSnapshotTransportConfig {
    fn default() -> Self {
        Self {
            connect_timeout: CONNECT_TIMEOUT,
            operation_timeout: OPERATION_TIMEOUT,
            response_idle_timeout: BACKUP_IDLE_TIMEOUT,
            max_diagnostic_bytes: 8 * 1024,
        }
    }
}

impl AdminSnapshotTransportConfig {
    /// How long establishing the TCP/TLS connection may take.
    pub fn with_connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = connect_timeout;
        self
    }

    /// The bound on one whole request, from send to the last body byte.
    pub fn with_operation_timeout(mut self, operation_timeout: Duration) -> Self {
        self.operation_timeout = operation_timeout;
        self
    }

    /// How long the transport waits for the next response chunk.
    pub fn with_response_idle_timeout(mut self, response_idle_timeout: Duration) -> Self {
        self.response_idle_timeout = response_idle_timeout;
        self
    }

    /// How many bytes of a non-200 response body a diagnostic keeps.
    pub fn with_max_diagnostic_bytes(mut self, max_diagnostic_bytes: usize) -> Self {
        self.max_diagnostic_bytes = max_diagnostic_bytes;
        self
    }

    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    pub fn operation_timeout(&self) -> Duration {
        self.operation_timeout
    }

    pub fn response_idle_timeout(&self) -> Duration {
        self.response_idle_timeout
    }

    pub fn max_diagnostic_bytes(&self) -> usize {
        self.max_diagnostic_bytes
    }
}
