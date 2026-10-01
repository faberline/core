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
    max_concurrent_streams: u32,
    drain_timeout: Duration,
    connection_budget: Option<ConnectionBudget>,
    drain: DrainController,
    socket: TcpSocketOptions,
    connection_metrics: Arc<dyn ConnectionMetrics>,
}

impl HttpServerOptions {
    /// Sets the HTTP/2 stream cap per connection.
    pub fn with_max_concurrent_streams(mut self, max_concurrent_streams: u32) -> Self {
        self.max_concurrent_streams = max_concurrent_streams;
        self
    }

    /// Sets how long open connections get to finish on the legacy drain path.
    pub fn with_drain_timeout(mut self, drain_timeout: Duration) -> Self {
        self.drain_timeout = drain_timeout;
        self
    }

    /// Caps the number of concurrent connections.
    pub fn with_connection_budget(mut self, budget: ConnectionBudget) -> Self {
        self.connection_budget = Some(budget);
        self
    }

    /// Sets the drain flag the legacy listener starts on shutdown.
    pub fn with_drain(mut self, drain: DrainController) -> Self {
        self.drain = drain;
        self
    }

    /// Sets the listen-socket options.
    pub fn with_socket(mut self, socket: TcpSocketOptions) -> Self {
        self.socket = socket;
        self
    }

    /// Sets the sink for connection events.
    pub fn with_connection_metrics(
        mut self,
        connection_metrics: Arc<dyn ConnectionMetrics>,
    ) -> Self {
        self.connection_metrics = connection_metrics;
        self
    }

    /// Changes the drain timeout of options already built.
    pub fn set_drain_timeout(&mut self, drain_timeout: Duration) {
        self.drain_timeout = drain_timeout;
    }

    /// The HTTP/2 stream cap per connection.
    pub fn max_concurrent_streams(&self) -> u32 {
        self.max_concurrent_streams
    }

    /// How long open connections get to finish on the legacy drain path.
    pub fn drain_timeout(&self) -> Duration {
        self.drain_timeout
    }

    /// The cap on concurrent connections, if any.
    pub fn connection_budget(&self) -> Option<&ConnectionBudget> {
        self.connection_budget.as_ref()
    }

    /// The drain flag the legacy listener starts on shutdown.
    pub fn drain(&self) -> &DrainController {
        &self.drain
    }

    /// The listen-socket options.
    pub fn socket(&self) -> TcpSocketOptions {
        self.socket
    }

    /// The sink for connection events.
    pub fn connection_metrics(&self) -> &Arc<dyn ConnectionMetrics> {
        &self.connection_metrics
    }
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
