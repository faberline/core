use serde::{Deserialize, Serialize};

use super::ids::{Index, NodeId};

/// Why a leader refused a promotion request (#3570).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromotionRefused {
    NotLeader,
    NotCaughtUp { matched: Index, target: Index },
    TransitionInFlight,
}

/// Why a leader refused a leadership transfer request (#3571).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferRefused {
    NotLeader,
    NotAVoter {
        target: NodeId,
    },
    NotCaughtUp {
        target: NodeId,
        matched: Index,
        last_index: Index,
    },
}

/// Why a leader refused a demotion request (#3572).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DemotionRefused {
    NotLeader,
    IsTheLeader { target: NodeId },
    TransitionInFlight,
    NotAVoter { target: NodeId },
    ToleranceWouldDrop { before: usize, after: usize },
    WouldEmptyVoterSet { target: NodeId },
}

/// Why a leader refused a removal request (#3572).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemovalRefused {
    NotLeader,
    TransitionInFlight,
    NotAMember { target: NodeId },
    IsTheLeader { target: NodeId },
    ToleranceWouldDrop { before: usize, after: usize },
    WouldEmptyVoterSet { target: NodeId },
}
