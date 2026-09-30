use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use server_lifecycle::{
    BindConfig, ConnectionBudget, ConnectionMetrics, DrainController, NoopConnectionMetrics,
};

#[derive(Clone)]
pub struct TcpServerConfig {
    bind: BindConfig,
    connection_budget: Option<ConnectionBudget>,
    drain: DrainController,
    socket: TcpSocketOptions,
    drain_timeout: Duration,
    connection_metrics: Arc<dyn ConnectionMetrics>,
}

impl fmt::Debug for TcpServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpServerConfig")
            .field("bind", &self.bind)
            .field("connection_budget", &self.connection_budget)
            .field("drain", &self.drain)
            .field("socket", &self.socket)
            .field("drain_timeout", &self.drain_timeout)
            .field("connection_metrics", &"dyn ConnectionMetrics")
            .finish()
    }
}

impl TcpServerConfig {
    pub fn new(bind: BindConfig) -> Self {
        Self {
            bind,
            connection_budget: None,
            drain: DrainController::new(),
            socket: TcpSocketOptions::default(),
            drain_timeout: Duration::from_secs(5),
            connection_metrics: Arc::new(NoopConnectionMetrics),
        }
    }

    pub fn with_connection_budget(mut self, budget: ConnectionBudget) -> Self {
        self.connection_budget = Some(budget);
        self
    }

    pub fn with_socket_options(mut self, socket: TcpSocketOptions) -> Self {
        self.socket = socket;
        self
    }

    pub fn with_drain_timeout(mut self, drain_timeout: Duration) -> Self {
        self.drain_timeout = drain_timeout;
        self
    }

    pub fn with_drain(mut self, drain: DrainController) -> Self {
        self.drain = drain;
        self
    }

    pub fn with_connection_metrics(mut self, metrics: Arc<dyn ConnectionMetrics>) -> Self {
        self.connection_metrics = metrics;
        self
    }

    /// The address the listener binds.
    pub fn bind(&self) -> &BindConfig {
        &self.bind
    }

    /// The cap on concurrent connections, if any.
    pub fn connection_budget(&self) -> Option<&ConnectionBudget> {
        self.connection_budget.as_ref()
    }

    /// The drain flag the listener starts on shutdown.
    pub fn drain(&self) -> &DrainController {
        &self.drain
    }

    /// The listen-socket options.
    pub fn socket(&self) -> TcpSocketOptions {
        self.socket
    }

    /// How long open connections get to finish when no lifecycle deadline
    /// applies.
    pub fn drain_timeout(&self) -> Duration {
        self.drain_timeout
    }

    /// The sink for accepted, rejected and closed connection events.
    pub fn connection_metrics(&self) -> &Arc<dyn ConnectionMetrics> {
        &self.connection_metrics
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpSocketOptions {
    backlog: i32,
    reuse_addr: bool,
    nodelay: bool,
}

impl TcpSocketOptions {
    /// Sets the listen backlog.
    pub fn with_backlog(mut self, backlog: i32) -> Self {
        self.backlog = backlog;
        self
    }

    /// Sets whether the listen socket reuses its address.
    pub fn with_reuse_addr(mut self, reuse_addr: bool) -> Self {
        self.reuse_addr = reuse_addr;
        self
    }

    /// Sets whether accepted connections disable Nagle's algorithm.
    pub fn with_nodelay(mut self, nodelay: bool) -> Self {
        self.nodelay = nodelay;
        self
    }

    /// The listen backlog.
    pub fn backlog(&self) -> i32 {
        self.backlog
    }

    /// Whether the listen socket reuses its address.
    pub fn reuse_addr(&self) -> bool {
        self.reuse_addr
    }

    /// Whether accepted connections disable Nagle's algorithm.
    pub fn nodelay(&self) -> bool {
        self.nodelay
    }
}

impl Default for TcpSocketOptions {
    fn default() -> Self {
        Self {
            backlog: 1024,
            reuse_addr: true,
            nodelay: true,
        }
    }
}
