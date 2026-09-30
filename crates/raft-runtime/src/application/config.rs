//! [`RaftHost`](crate::RaftHost) tuning.

use std::time::Duration;

/// When the host captures a state-machine snapshot and compacts the raft log.
#[derive(Clone, Copy, Debug)]
pub enum SnapshotPolicy {
    /// Never compact (the log grows; fine for log-broker state machines with no
    /// meaningful snapshot, e.g. relay).
    Disabled,
    /// Compact when `applied_index - snapshot_index >= n`.
    EveryEntries(u64),
    /// The host never auto-compacts; the consumer drives it (e.g. lumen's
    /// periodic RDB snapshotter calls `snapshot_and_compact`).
    External,
}

/// Host timing + snapshot policy.
///
/// Start from [`HostConfig::default`] and change what you need with the
/// `with_*` builders.
#[derive(Clone, Copy, Debug)]
pub struct HostConfig {
    tick: Duration,
    pump: Duration,
    rpc_timeout: Duration,
    propose_timeout: Duration,
    snapshot: SnapshotPolicy,
}

impl HostConfig {
    /// Logical tick (election/heartbeat clock).
    pub fn tick(&self) -> Duration {
        self.tick
    }

    /// Fast outbox pump (ships replies-driven work under the election timeout).
    pub fn pump(&self) -> Duration {
        self.pump
    }

    /// Peer RPC timeout.
    pub fn rpc_timeout(&self) -> Duration {
        self.rpc_timeout
    }

    /// How long `propose` waits for its entry to apply before erroring.
    pub fn propose_timeout(&self) -> Duration {
        self.propose_timeout
    }

    /// Auto-compaction policy.
    pub fn snapshot(&self) -> SnapshotPolicy {
        self.snapshot
    }

    /// Set the logical tick.
    pub fn with_tick(mut self, tick: Duration) -> Self {
        self.tick = tick;
        self
    }

    /// Set the outbox pump interval.
    pub fn with_pump(mut self, pump: Duration) -> Self {
        self.pump = pump;
        self
    }

    /// Set the peer RPC timeout.
    pub fn with_rpc_timeout(mut self, rpc_timeout: Duration) -> Self {
        self.rpc_timeout = rpc_timeout;
        self
    }

    /// Set how long `propose` waits for its entry to apply.
    pub fn with_propose_timeout(mut self, propose_timeout: Duration) -> Self {
        self.propose_timeout = propose_timeout;
        self
    }

    /// Set the auto-compaction policy.
    pub fn with_snapshot(mut self, snapshot: SnapshotPolicy) -> Self {
        self.snapshot = snapshot;
        self
    }
}

impl Default for HostConfig {
    fn default() -> Self {
        HostConfig {
            tick: Duration::from_millis(20),
            pump: Duration::from_millis(5),
            rpc_timeout: Duration::from_millis(400),
            propose_timeout: Duration::from_secs(10),
            snapshot: SnapshotPolicy::Disabled,
        }
    }
}
