use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use server_lifecycle::{
    ConnectionBudget, ConnectionMetrics, DrainController, NoopConnectionMetrics,
};
use server_tcp::TcpSocketOptions;

/// HTTP listener/runtime options owned by `server-http`.
#[derive(Clone)]
pub struct HttpServerOptions {
    pub max_concurrent_streams: u32,
    pub drain_timeout: Duration,
    pub connection_budget: Option<ConnectionBudget>,
    pub drain: DrainController,
    pub socket: TcpSocketOptions,
    pub connection_metrics: Arc<dyn ConnectionMetrics>,
}

impl Default for HttpServerOptions {
    fn default() -> Self {
        Self {
            max_concurrent_streams: 4096,
            drain_timeout: Duration::from_secs(5),
            connection_budget: None,
            drain: DrainController::new(),
            socket: TcpSocketOptions::default(),
            connection_metrics: Arc::new(NoopConnectionMetrics),
        }
    }
}

impl fmt::Debug for HttpServerOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpServerOptions")
            .field("max_concurrent_streams", &self.max_concurrent_streams)
            .field("drain_timeout", &self.drain_timeout)
            .field("connection_budget", &self.connection_budget)
            .field("drain", &self.drain)
            .field("socket", &self.socket)
            .field("connection_metrics", &"dyn ConnectionMetrics")
            .finish()
    }
}

/// Backwards-compatible name for existing service runtime plans.
pub type H2cServerOptions = HttpServerOptions;
