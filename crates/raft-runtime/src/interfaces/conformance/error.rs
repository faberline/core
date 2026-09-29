use std::fmt;

use raft_core::{DemotionRefused, NodeId, PromotionRefused, RemovalRefused};

/// Membership operation failures that a conformance trace may expect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConformanceMembershipError {
    NotLeader,
    AddLearnerRefused,
    Promote(PromotionRefused),
    Demote(DemotionRefused),
    Remove(RemovalRefused),
}

impl fmt::Display for ConformanceMembershipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ConformanceMembershipError {}

/// Errors returned by deterministic host actions.  Store errors carry only a
/// stable operation and kind; the original `io::Error` is deliberately not
/// retained because it is platform text, not conformance state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepError {
    Store {
        operation: &'static str,
        kind: std::io::ErrorKind,
    },
    StateMachine {
        operation: &'static str,
        message: String,
    },
    WrongRecipient {
        expected: NodeId,
        actual: NodeId,
    },
    NotLeader,
    Membership(ConformanceMembershipError),
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StepError {}
