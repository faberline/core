/// Logical ticks before a voter starts an election (distinct per node so the
/// deterministic simulation does not livelock on split votes).
// With raft-runtime's 20ms production tick this is a one-second minimum.
// Stateful adopters persist proposals before releasing the node lock; a
// 200ms election window made ordinary bursts of durable fsyncs look like a
// failed leader and caused avoidable term churn.
pub const ELECTION_TIMEOUT_FLOOR_TICKS: u64 = 50;
/// Ticks between leader heartbeats / replication pushes.
pub const HEARTBEAT_INTERVAL_TICKS: u64 = 3;
