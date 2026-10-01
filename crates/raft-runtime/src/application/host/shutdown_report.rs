use super::*;

/// The outcome of an attempt to hand off leadership before shutdown (#3664).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeadershipHandoff {
    Transferred { target: NodeId },
    NotLeader,
    SoleVoter,
    NoCaughtUpVoter { voters: usize },
}

/// The four sequential phases of host shutdown (#3672).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShutdownPhase {
    Quiesce,
    LeadershipHandoff,
    BackgroundTasks,
    PeerRpcDrain,
}

/// The status of an individual shutdown phase (#3672).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseStatus {
    Completed,
    DeadlineExpired,
    StorageFailed,
}

/// A record of one shutdown phase execution (#3672).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseRecord {
    pub phase: ShutdownPhase,
    pub status: PhaseStatus,
    pub elapsed: Duration,
}

/// Which caller role this report represents in host shutdown (#3683).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShutdownCaller {
    Executed,
    Joined,
}

/// The terminal report returned by `shutdown_within` (#3672, #3683).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostShutdownReport {
    pub caller: ShutdownCaller,
    pub phases: Vec<PhaseRecord>,
    pub handoff: LeadershipHandoff,
    pub incomplete_phase: Option<ShutdownPhase>,
    pub peer_listener_close_safe: bool,
    pub storage_failure: Option<StorageFailed>,
}

impl HostShutdownReport {
    /// Convert this shutdown report into a `Result<()>`.
    ///
    /// Returns `Ok(())` if shutdown completed cleanly. Returns `Err` if a storage
    /// failure was observed or if shutdown stopped early in any phase.
    pub fn into_result(self) -> Result<()> {
        if let Some(failure) = self.storage_failure {
            return Err(anyhow!("{failure}"));
        }
        if let Some(phase) = self.incomplete_phase {
            return Err(anyhow!("raft: shutdown stopped early in phase {phase:?}"));
        }
        Ok(())
    }
}
