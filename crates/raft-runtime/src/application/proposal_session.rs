//! Per-process proposal ids: a session stamp plus a sequence counter, so that
//! two proposals from one process never share an id.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use raft_core::NodeId;

/// The `(node, session, sequence)` triple that identifies one proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProposalStamp {
    node: NodeId,
    session: u64,
    sequence: u64,
}

impl ProposalStamp {
    /// The node that made the proposal.
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// The session of the process that made the proposal.
    pub const fn session(&self) -> u64 {
        self.session
    }

    /// The sequence number, unique within its session.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
}

/// Hands out [`ProposalStamp`]s for one node in one process session.
#[derive(Debug)]
pub struct ProposalSession {
    node: NodeId,
    session: u64,
    next: AtomicU64,
}

impl ProposalSession {
    /// A session with a fresh id: wall-clock nanos, the process id and a
    /// per-process counter, so two sessions in one process differ.
    pub fn new(node: NodeId) -> Self {
        static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let session = wall
            ^ ((std::process::id() as u64) << 32)
            ^ NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
        Self::with_session(node, session)
    }

    /// A session with a fixed session id.
    pub fn with_session(node: NodeId, session: u64) -> Self {
        Self {
            node,
            session,
            next: AtomicU64::new(1),
        }
    }

    /// The node this session stamps for.
    pub fn node(&self) -> NodeId {
        self.node
    }

    /// The session id.
    pub fn session(&self) -> u64 {
        self.session
    }

    /// The next stamp; sequences start at 1 and never repeat.
    pub fn next_stamp(&self) -> ProposalStamp {
        ProposalStamp {
            node: self.node,
            session: self.session,
            sequence: self.next.fetch_add(1, Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Arc;

    #[test]
    fn stamps_count_from_one_in_order() {
        let session = ProposalSession::with_session(NodeId::new(7), 42);
        let first = session.next_stamp();
        let second = session.next_stamp();
        assert_eq!(first.sequence(), 1);
        assert_eq!(second.sequence(), 2);
        assert_eq!(first.node(), NodeId::new(7));
        assert_eq!(second.node(), NodeId::new(7));
        assert_eq!(first.session(), 42);
        assert_eq!(second.session(), 42);
    }

    #[test]
    fn stamps_are_unique_across_threads() {
        let session = Arc::new(ProposalSession::with_session(NodeId::new(3), 9));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let session = Arc::clone(&session);
                std::thread::spawn(move || {
                    (0..250)
                        .map(|_| session.next_stamp().sequence())
                        .collect::<Vec<u64>>()
                })
            })
            .collect();
        let mut seen = HashSet::new();
        for handle in handles {
            seen.extend(handle.join().unwrap());
        }
        assert_eq!(seen.len(), 1000);
    }

    #[test]
    fn new_sessions_differ() {
        let a = ProposalSession::new(NodeId::new(1)).session();
        let b = ProposalSession::new(NodeId::new(1)).session();
        assert_ne!(a, b);
    }
}
