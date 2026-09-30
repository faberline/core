//! Self-contained Raft consensus core (no external dependency).
//!
//! [`RaftNode`] is **step-driven**: it never spawns timers or threads. A driver
//! calls [`tick`](RaftNode::tick) to advance logical time and
//! [`handle`](RaftNode::handle) to feed it an incoming [`RaftMsg`]; the node
//! accumulates replies/heartbeats in an outbox drained via
//! [`take_outgoing`](RaftNode::take_outgoing). This makes the whole protocol a
//! deterministic state machine that a test can simulate exactly (no real
//! network / clock), and a production driver can wrap with an h2c transport.
//!
//! Replicated-state-machine model: the Raft log holds opaque **command** bytes.
//! Once an entry commits (acked by a majority of voters), every node surfaces it
//! via [`take_committed`](RaftNode::take_committed) for the consumer to apply to
//! its own state machine.
//!
//! **Snapshots / log compaction** keep the log bounded for large state machines:
//! a consumer that has applied up to some index snapshots its state machine and
//! calls [`compact`](RaftNode::compact); the Raft log before that index is
//! dropped. A leader replicating to a follower whose next index has been
//! compacted away ships the snapshot (`InstallSnapshot`) instead of replaying
//! the whole history; the follower installs it and surfaces the bytes via
//! [`take_installed_snapshot`](RaftNode::take_installed_snapshot).

mod domain;

pub use domain::{
    auto_membership, AppendReq, AppendResp, ConfState, DemotionRefused, EntryKind, Index,
    InstallSnapshotReq, InstallSnapshotResp, Membership, NodeId, Outgoing, PersistedState,
    PersistedStateRef, PromotionRefused, RaftEntry, RaftMsg, RaftNode, RemovalRefused, Role, Term,
    TimeoutNowReq, TransferRefused, VoteReq, VoteResp, ELECTION_TIMEOUT_FLOOR_TICKS,
    HEARTBEAT_INTERVAL_TICKS,
};
