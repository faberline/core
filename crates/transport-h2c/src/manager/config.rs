use std::time::Duration;

use crate::conn::ConnConfig;
use crate::recommended_h2c_connections;

/// Configuration for an [`H2cManager`](super::H2cManager).
#[derive(Clone, Debug)]
pub struct ManagerConfig {
    /// Connections kept warm at all times (opened eagerly at connect).
    min_connections: usize,
    /// Hard ceiling on h2 connections (adaptive growth stops here).
    max_connections: usize,
    /// Idle h2 connections retained after bursts. HTTP/2 normally stays below
    /// this because connection count is logarithmic in target concurrency.
    max_keepalive_connections: usize,
    /// Hard cap on requests admitted into this manager at once. Additional
    /// callers queue until a slot is released or `pool_timeout` elapses.
    max_in_flight_per_origin: usize,
    /// Grow a new connection when the least-loaded healthy one has at least this
    /// many in-flight streams (and we're under `max_connections`).
    grow_threshold: usize,
    /// Deadline for waiting on an admission slot.
    pool_timeout: Duration,
    /// Deadline for a single TCP connect + handshake.
    connect_timeout: Duration,
    /// Per-request deadline (`None` disables it).
    request_timeout: Option<Duration>,
    /// Supervisor cadence: liveness ping + prune/shrink/replenish sweep.
    ping_interval: Duration,
    /// Shrink a connection idle longer than this (above `min_connections`).
    idle_timeout: Duration,
    /// h2 per-stream receive window.
    stream_window: u32,
    /// h2 whole-connection receive window.
    conn_window: u32,
    /// h2 max frame size.
    max_frame: u32,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: recommended_h2c_connections(128).max(1),
            max_keepalive_connections: 16,
            max_in_flight_per_origin: 128,
            grow_threshold: 32,
            pool_timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Some(Duration::from_secs(30)),
            ping_interval: Duration::from_secs(15),
            idle_timeout: Duration::from_secs(5),
            stream_window: 1024 * 1024,   // 1 MiB
            conn_window: 4 * 1024 * 1024, // 4 MiB
            max_frame: 16 * 1024,         // 16 KiB
        }
    }
}

impl ManagerConfig {
    /// Cap `max_connections` by the `ln(concurrency)`/cores heuristic for a
    /// target peak concurrency, and use the same value as the request-admission
    /// hard cap.
    pub fn for_concurrency(concurrency: usize) -> Self {
        let mut c = Self::default();
        c.max_connections = recommended_h2c_connections(concurrency).max(c.min_connections);
        c.max_in_flight_per_origin = concurrency.max(1);
        c
    }

    /// Sets the connections kept warm at all times.
    pub fn with_min_connections(mut self, min_connections: usize) -> Self {
        self.min_connections = min_connections;
        self
    }

    /// Sets the hard ceiling on h2 connections.
    pub fn with_max_connections(mut self, max_connections: usize) -> Self {
        self.max_connections = max_connections;
        self
    }

    /// Sets the idle h2 connections retained after bursts.
    pub fn with_max_keepalive_connections(mut self, max_keepalive_connections: usize) -> Self {
        self.max_keepalive_connections = max_keepalive_connections;
        self
    }

    /// Sets the hard cap on requests admitted at once.
    pub fn with_max_in_flight_per_origin(mut self, max_in_flight_per_origin: usize) -> Self {
        self.max_in_flight_per_origin = max_in_flight_per_origin;
        self
    }

    /// Sets the in-flight streams that trigger growth.
    pub fn with_grow_threshold(mut self, grow_threshold: usize) -> Self {
        self.grow_threshold = grow_threshold;
        self
    }

    /// Sets the deadline for waiting on an admission slot.
    pub fn with_pool_timeout(mut self, pool_timeout: Duration) -> Self {
        self.pool_timeout = pool_timeout;
        self
    }

    /// Sets the deadline for a single TCP connect and handshake.
    pub fn with_connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = connect_timeout;
        self
    }

    /// Sets the per-request deadline; `None` disables it.
    pub fn with_request_timeout(mut self, request_timeout: Option<Duration>) -> Self {
        self.request_timeout = request_timeout;
        self
    }

    /// Sets the supervisor cadence.
    pub fn with_ping_interval(mut self, ping_interval: Duration) -> Self {
        self.ping_interval = ping_interval;
        self
    }

    /// Sets how long a connection may idle before it is shrunk.
    pub fn with_idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.idle_timeout = idle_timeout;
        self
    }

    /// Sets the h2 per-stream receive window.
    pub fn with_stream_window(mut self, stream_window: u32) -> Self {
        self.stream_window = stream_window;
        self
    }

    /// Sets the h2 whole-connection receive window.
    pub fn with_conn_window(mut self, conn_window: u32) -> Self {
        self.conn_window = conn_window;
        self
    }

    /// Sets the h2 max frame size.
    pub fn with_max_frame(mut self, max_frame: u32) -> Self {
        self.max_frame = max_frame;
        self
    }

    /// Connections kept warm at all times.
    pub fn min_connections(&self) -> usize {
        self.min_connections
    }

    /// The hard ceiling on h2 connections.
    pub fn max_connections(&self) -> usize {
        self.max_connections
    }

    /// Idle h2 connections retained after bursts.
    pub fn max_keepalive_connections(&self) -> usize {
        self.max_keepalive_connections
    }

    /// The hard cap on requests admitted at once.
    pub fn max_in_flight_per_origin(&self) -> usize {
        self.max_in_flight_per_origin
    }

    /// In-flight streams on the least-loaded connection that trigger growth.
    pub fn grow_threshold(&self) -> usize {
        self.grow_threshold
    }

    /// The deadline for waiting on an admission slot.
    pub fn pool_timeout(&self) -> Duration {
        self.pool_timeout
    }

    /// The deadline for a single TCP connect and handshake.
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// The per-request deadline; `None` disables it.
    pub fn request_timeout(&self) -> Option<Duration> {
        self.request_timeout
    }

    /// The supervisor cadence.
    pub fn ping_interval(&self) -> Duration {
        self.ping_interval
    }

    /// How long a connection above `min_connections` may idle before it is shrunk.
    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// The h2 per-stream receive window.
    pub fn stream_window(&self) -> u32 {
        self.stream_window
    }

    /// The h2 whole-connection receive window.
    pub fn conn_window(&self) -> u32 {
        self.conn_window
    }

    /// The h2 max frame size.
    pub fn max_frame(&self) -> u32 {
        self.max_frame
    }

    pub(super) fn conn_config(&self) -> ConnConfig {
        ConnConfig {
            stream_window: self.stream_window,
            conn_window: self.conn_window,
            max_frame: self.max_frame,
        }
    }
}

/// Aggregate snapshot of an [`H2cManager`](super::H2cManager).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ManagerStats {
    /// Connections currently in the pool (healthy + not-yet-pruned dead).
    pub connections: usize,
    /// Of those, how many are healthy.
    pub healthy: usize,
    /// In-flight streams summed across connections.
    pub in_flight: usize,
    /// Lifetime requests started (including on since-evicted connections).
    pub total_requests: u64,
    /// Lifetime errors (including on since-evicted connections).
    pub total_errors: u64,
}
