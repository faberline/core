use std::time::Duration;

use crate::conn::ConnConfig;
use crate::recommended_h2c_connections;

/// Configuration for an [`H2cManager`](super::H2cManager).
#[derive(Clone, Debug)]
pub struct ManagerConfig {
    /// Connections kept warm at all times (opened eagerly at connect).
    pub min_connections: usize,
    /// Hard ceiling on h2 connections (adaptive growth stops here).
    pub max_connections: usize,
    /// Idle h2 connections retained after bursts. HTTP/2 normally stays below
    /// this because connection count is logarithmic in target concurrency.
    pub max_keepalive_connections: usize,
    /// Hard cap on requests admitted into this manager at once. Additional
    /// callers queue until a slot is released or `pool_timeout` elapses.
    pub max_in_flight_per_origin: usize,
    /// Grow a new connection when the least-loaded healthy one has at least this
    /// many in-flight streams (and we're under `max_connections`).
    pub grow_threshold: usize,
    /// Deadline for waiting on an admission slot.
    pub pool_timeout: Duration,
    /// Deadline for a single TCP connect + handshake.
    pub connect_timeout: Duration,
    /// Per-request deadline (`None` disables it).
    pub request_timeout: Option<Duration>,
    /// Supervisor cadence: liveness ping + prune/shrink/replenish sweep.
    pub ping_interval: Duration,
    /// Shrink a connection idle longer than this (above `min_connections`).
    pub idle_timeout: Duration,
    /// h2 per-stream receive window.
    pub stream_window: u32,
    /// h2 whole-connection receive window.
    pub conn_window: u32,
    /// h2 max frame size.
    pub max_frame: u32,
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
