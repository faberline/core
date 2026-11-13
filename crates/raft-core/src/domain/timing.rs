use crate::domain::ids::NodeId;

/// Logical ticks before a voter starts an election (distinct per node so the
/// deterministic simulation does not livelock on split votes).
// With raft-runtime's 20ms production tick this is a one-second minimum.
// Stateful adopters persist proposals before releasing the node lock; a
// 200ms election window made ordinary bursts of durable fsyncs look like a
// failed leader and caused avoidable term churn.
pub const ELECTION_TIMEOUT_FLOOR_TICKS: u64 = 50;
/// Ticks between leader heartbeats / replication pushes.
pub const HEARTBEAT_INTERVAL_TICKS: u64 = 3;
/// Gap in ticks between the first election timeouts of two adjacent node ids.
/// Must be longer than one vote round trip, so the first candidate wins before
/// the next node times out.
pub(crate) const ELECTION_STAGGER_TICKS: u64 = 5;

/// First election timeout of node `id`: the floor plus its id times the stagger.
pub(crate) fn base_election_timeout_ticks(id: NodeId) -> u64 {
    ELECTION_TIMEOUT_FLOOR_TICKS.saturating_add(id.get().saturating_mul(ELECTION_STAGGER_TICKS))
}
