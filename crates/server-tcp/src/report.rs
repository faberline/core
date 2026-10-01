use crate::connection::{TcpConnectionResult, TcpConnectionTerminal};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TcpServerReport {
    pub accepted: u64,
    pub rejected: u64,
    pub completed: u64,
    pub failed: u64,
    pub timed_out: u64,
    pub unfinished: u64,
    pub streams_completed: u64,
    pub streams_admitted: u64,
    pub streams_active_at_drain: u64,
    pub streams_refused: u64,
    pub streams_timed_out: u64,
    pub streams_ambiguous: u64,
    pub accept_errors: u64,
    pub deadline_missing: bool,
}

impl TcpServerReport {
    pub(crate) fn record(&mut self, result: TcpConnectionResult) {
        match result.terminal() {
            TcpConnectionTerminal::Completed => self.completed += 1,
            TcpConnectionTerminal::Failed => self.failed += 1,
            TcpConnectionTerminal::TimedOut => self.timed_out += 1,
        }
        self.streams_completed += result.streams_completed();
        self.streams_admitted += result.streams_admitted();
        self.streams_active_at_drain += result.streams_active_at_drain();
        self.streams_refused += result.streams_refused();
        self.streams_timed_out += result.streams_timed_out();
        self.streams_ambiguous += result.streams_ambiguous();
    }
}
