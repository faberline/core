use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use server_lifecycle::{
    BindConfig, ConnectionBudget, ConnectionMetrics, DrainController, NoopConnectionMetrics,
};

#[derive(Clone)]
pub struct TcpServerConfig {
    pub bind: BindConfig,
    pub connection_budget: Option<ConnectionBudget>,
    pub drain: DrainController,
    pub socket: TcpSocketOptions,
    pub drain_timeout: Duration,
    pub connection_metrics: Arc<dyn ConnectionMetrics>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpSocketOptions {
    pub backlog: i32,
    pub reuse_addr: bool,
    pub nodelay: bool,
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
