use super::*;

/// Result of one externally coordinated snapshot attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotCompactionOutcome {
    pub snapshot_index: Index,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageFailed {
    pub node_id: NodeId,
    pub operation: &'static str,
    pub path: std::path::PathBuf,
    pub kind: std::io::ErrorKind,
}

impl std::fmt::Display for StorageFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "durable storage failed for node {} on {} at {}: {:?}",
            self.node_id,
            self.operation,
            self.path.display(),
            self.kind
        )
    }
}

impl std::error::Error for StorageFailed {}

/// Terminal outcome of a Raft proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalOutcome {
    Completed {
        index: Index,
    },
    RejectedBeforeAdmission {
        reason: String,
    },
    Ambiguous {
        index: Option<Index>,
        reason: String,
    },
    DurabilityFailure {
        index: Option<Index>,
        failure: StorageFailed,
    },
}
