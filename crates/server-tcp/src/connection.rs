use std::net::SocketAddr;

use server_lifecycle::{DrainSignal, LifecycleSubscription};

#[derive(Debug, Clone)]
pub struct ConnectionContext {
    pub local_addr: SocketAddr,
    pub peer_addr: SocketAddr,
    pub drain: DrainSignal,
    /// Authoritative lifecycle subscription for this accepted connection.
    pub lifecycle: LifecycleSubscription,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpConnectionTerminal {
    Completed,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpConnectionResult {
    pub terminal: TcpConnectionTerminal,
    pub streams_admitted: u64,
    pub streams_active_at_drain: u64,
    pub streams_completed: u64,
    pub streams_refused: u64,
    pub streams_timed_out: u64,
    pub streams_ambiguous: u64,
}

impl Default for TcpConnectionResult {
    fn default() -> Self {
        Self {
            terminal: TcpConnectionTerminal::Completed,
            streams_admitted: 0,
            streams_active_at_drain: 0,
            streams_completed: 0,
            streams_refused: 0,
            streams_timed_out: 0,
            streams_ambiguous: 0,
        }
    }
}
