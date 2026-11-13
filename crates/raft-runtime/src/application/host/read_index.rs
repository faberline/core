use super::*;

use crate::domain::ReadIndexReply;
use raft_core::ReadRefused;

use super::propose::Route;

/// Why `RaftHost::read_index` gave no index (#109).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadIndexError {
    /// A peer answered that it is not the leader, and no leader was reached in time.
    NotLeader,
    /// The leader has not committed an entry of its own term yet.
    NoCommitInTerm,
    /// This node stopped being leader before a quorum acked the round.
    LeadershipLost,
    /// No quorum acked the round within `HostConfig::propose_timeout`.
    TimedOut,
    /// No leader known within `HostConfig::propose_timeout`.
    NoLeader,
    /// The request to the leader failed or its reply was not understood.
    ForwardFailed,
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
            ReadIndexError::NoLeader => write!(f, "no leader known"),
            ReadIndexError::ForwardFailed => {
                write!(f, "forwarding the read to the leader failed")
            }
        }
    }
}

impl std::error::Error for ReadIndexError {}

impl ReadIndexError {
    /// Map the leader's `503` reason text back to the error it names. Any
    /// other text is a failure of the forward itself.
    pub(crate) fn from_leader_reason(reason: &str) -> ReadIndexError {
        [
            ReadIndexError::NoCommitInTerm,
            ReadIndexError::LeadershipLost,
            ReadIndexError::TimedOut,
        ]
        .into_iter()
        .find(|e| e.to_string() == reason)
        .unwrap_or(ReadIndexError::ForwardFailed)
    }
}

impl RaftHost {
    /// Linearizable read index (Raft section 6.4). Works on the leader and on
    /// a follower, which forwards the round to the leader.
    ///
    /// Returns the commit index seen when the call started, after a quorum
    /// confirmed the leader is still leader. The caller then waits for
    /// `applied_watch()` to reach the returned index. No log entry is appended.
    pub async fn read_index(&self) -> std::result::Result<Index, ReadIndexError> {
        let s = &self.shared;
        let deadline = Instant::now() + s.cfg.propose_timeout();
        let mut last_failed = false;
        loop {
            let route = {
                let n = s.node.lock().await;
                if n.is_leader() {
                    Route::Local
                } else {
                    match s.leader_url(&n).1 {
                        Some(url) => Route::Remote(url),
                        None => Route::Unknown,
                    }
                }
            };
            match route {
                Route::Local => match s.read_index_leader().await {
                    // Lost leadership between the check and the call: route again.
                    Err(ReadIndexError::NotLeader) => {}
                    other => return other,
                },
                Route::Remote(url) => match s.peer_client.read_index(&url).await {
                    ReadIndexReply::Granted { index } => return Ok(Index::new(index)),
                    // Stale leader hint: route again.
                    ReadIndexReply::NotLeader => {}
                    ReadIndexReply::Refused { reason } => {
                        return Err(ReadIndexError::from_leader_reason(&reason))
                    }
                    ReadIndexReply::Failed { reason } => {
                        tracing::debug!(%reason, "read-index forward failed");
                        last_failed = true;
                    }
                },
                Route::Unknown => {}
            }
            if Instant::now() >= deadline {
                return Err(if last_failed {
                    ReadIndexError::ForwardFailed
                } else {
                    ReadIndexError::NoLeader
                });
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl Shared {
    /// The leader path of [`RaftHost::read_index`]. The peer read-index route
    /// (#187 part 2) calls it too, so both share one quorum round.
    pub(crate) async fn read_index_leader(
        self: &Arc<Self>,
    ) -> std::result::Result<Index, ReadIndexError> {
        let (round, index) = {
            let mut node = self.node.lock().await;
            let index = node.commit_index();
            match node.start_read_round() {
                Ok(round) => (round, index),
                Err(ReadRefused::NotLeader) => return Err(ReadIndexError::NotLeader),
                Err(ReadRefused::NoCommitInTerm) => return Err(ReadIndexError::NoCommitInTerm),
            }
        };
        self.flush().await;
        let deadline = Instant::now() + self.cfg.propose_timeout();
        loop {
            {
                let node = self.node.lock().await;
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
            tokio::time::sleep(self.cfg.pump()).await;
        }
    }
}
