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
    terminal: TcpConnectionTerminal,
    streams_admitted: u64,
    streams_active_at_drain: u64,
    streams_completed: u64,
    streams_refused: u64,
    streams_timed_out: u64,
    streams_ambiguous: u64,
}

impl TcpConnectionResult {
    /// A connection that ended as `terminal`, with every stream counter at
    /// zero.
    pub fn new(terminal: TcpConnectionTerminal) -> Self {
        Self {
            terminal,
            streams_admitted: 0,
            streams_active_at_drain: 0,
            streams_completed: 0,
            streams_refused: 0,
            streams_timed_out: 0,
            streams_ambiguous: 0,
        }
    }

    /// Sets the number of request streams admitted.
    pub fn with_streams_admitted(mut self, streams_admitted: u64) -> Self {
        self.streams_admitted = streams_admitted;
        self
    }

    /// Sets the number of streams still active when the drain started.
    pub fn with_streams_active_at_drain(mut self, streams_active_at_drain: u64) -> Self {
        self.streams_active_at_drain = streams_active_at_drain;
        self
    }

    /// Sets the number of streams that completed.
    pub fn with_streams_completed(mut self, streams_completed: u64) -> Self {
        self.streams_completed = streams_completed;
        self
    }

    /// Sets the number of streams refused.
    pub fn with_streams_refused(mut self, streams_refused: u64) -> Self {
        self.streams_refused = streams_refused;
        self
    }

    /// Sets the number of streams that timed out.
    pub fn with_streams_timed_out(mut self, streams_timed_out: u64) -> Self {
        self.streams_timed_out = streams_timed_out;
        self
    }

    /// Sets the number of streams whose outcome is unknown.
    pub fn with_streams_ambiguous(mut self, streams_ambiguous: u64) -> Self {
        self.streams_ambiguous = streams_ambiguous;
        self
    }

    /// How the connection ended.
    pub fn terminal(&self) -> TcpConnectionTerminal {
        self.terminal
    }

    /// The number of request streams admitted.
    pub fn streams_admitted(&self) -> u64 {
        self.streams_admitted
    }

    /// The number of streams still active when the drain started.
    pub fn streams_active_at_drain(&self) -> u64 {
        self.streams_active_at_drain
    }

    /// The number of streams that completed.
    pub fn streams_completed(&self) -> u64 {
        self.streams_completed
    }

    /// The number of streams refused.
    pub fn streams_refused(&self) -> u64 {
        self.streams_refused
    }

    /// The number of streams that timed out.
    pub fn streams_timed_out(&self) -> u64 {
        self.streams_timed_out
    }

    /// The number of streams whose outcome is unknown.
    pub fn streams_ambiguous(&self) -> u64 {
        self.streams_ambiguous
    }
}

impl Default for TcpConnectionResult {
    fn default() -> Self {
        Self::new(TcpConnectionTerminal::Completed)
    }
}
