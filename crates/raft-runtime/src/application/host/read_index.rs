use super::*;

use raft_core::ReadRefused;

/// Why `RaftHost::read_index` gave no index (#109).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadIndexError {
    /// This node is not the leader. Forwarding is #110.
    NotLeader,
    /// The leader has not committed an entry of its own term yet.
    NoCommitInTerm,
    /// This node stopped being leader before a quorum acked the round.
    LeadershipLost,
    /// No quorum acked the round within `HostConfig::propose_timeout`.
    TimedOut,
}

impl std::fmt::Display for ReadIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadIndexError::NotLeader => write!(f, "not the leader"),
            ReadIndexError::NoCommitInTerm => {
                write!(f, "leader has not committed an entry of its own term")
            }
            ReadIndexError::LeadershipLost => {
                write!(f, "leadership lost before a quorum confirmed the read")
            }
            ReadIndexError::TimedOut => write!(f, "no quorum confirmed the read in time"),
        }
    }
}

impl std::error::Error for ReadIndexError {}

impl RaftHost {
    /// Linearizable read index (Raft section 6.4, leader path).
    ///
    /// Returns the commit index seen when the call started, after a quorum
    /// confirmed this node is still leader. The caller then waits for
    /// `applied_watch()` to reach the returned index. No log entry is appended.
    pub async fn read_index(&self) -> std::result::Result<Index, ReadIndexError> {
        let (round, index) = {
            let mut node = self.shared.node.lock().await;
            let index = node.commit_index();
            match node.start_read_round() {
                Ok(round) => (round, index),
                Err(ReadRefused::NotLeader) => return Err(ReadIndexError::NotLeader),
                Err(ReadRefused::NoCommitInTerm) => return Err(ReadIndexError::NoCommitInTerm),
            }
        };
        self.shared.flush().await;
        let deadline = Instant::now() + self.shared.cfg.propose_timeout();
        loop {
            {
                let node = self.shared.node.lock().await;
                if node.read_round_acked(round) {
                    return Ok(index);
                }
                if !node.is_leader() {
                    return Err(ReadIndexError::LeadershipLost);
                }
            }
            if Instant::now() >= deadline {
                return Err(ReadIndexError::TimedOut);
            }
            tokio::time::sleep(self.shared.cfg.pump()).await;
        }
    }
}
