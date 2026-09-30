//! `raft-runtime` — the ecosystem's shared raft driver.
//!
//! `crates/raft-core` is the step-driven consensus core; this crate is the **host**
//! that drives it for a [`RaftStateMachine`]: a tick/pump loop, the h2c peer
//! transport (Vote / Append / InstallSnapshot), the single apply loop, snapshot
//! plus log compaction, a read-your-write [`RaftHost::propose`], and a peer
//! [`RaftHost::router`] to merge into the service's h2c port.
//!
//! Every raft_core service (lumen, keep, relay, loom) supplies a
//! [`RaftStateMachine`] (`apply`/`snapshot`/`restore`/`applied_index`) and gets
//! HA + the backup layer for free, instead of hand-rolling a driver.
//!
//! ## Cluster topology and auto-mode
//!
//! Every raft_core service derives the same thing from the StatefulSet downward
//! API: which mode to run (single-node vs replica/HA), this node's id, the
//! group membership, and the peer URLs. [`replica_mode`], [`ClusterTopology`],
//! [`ClusterDims`], [`peer_ordinal`], [`parse_peer_overrides`] and
//! [`ensure_static_membership_unchanged`] centralize it so services compose it
//! instead of hand-rolling the ordinal math + peer-DNS each time.

mod api;
mod application;
mod infrastructure;
mod interfaces;

#[cfg(test)]
mod tests;

pub use api::{conformance, llm};
pub use application::{
    ensure_static_membership_unchanged, peer_ordinal, ClusterDims, ClusterTopology,
};
pub use application::{
    ActiveAssignment, AssignmentEpoch, AssignmentError, FenceToken, FencedAssignment,
};
pub use application::{
    AdmissionPermit, Command, PreparedSnapshot, RaftStateMachine, SnapshotPreparation,
};
pub use application::{
    AdmissionRefused, ChunkSink, HostShutdownReport, LeadershipHandoff, MembershipPhase,
    PhaseRecord, PhaseStatus, ProposalBackpressure, ProposalOutcome, RaftHost, RaftStatus,
    ShutdownCaller, ShutdownPhase, SnapshotCompactionOutcome, StorageFailed, SNAPSHOT_CHUNK_SIZE,
};
pub use application::{GroupId, LEGACY_GROUP_ID};
pub use application::{HostConfig, SnapshotPolicy};
pub use application::{MembershipPolicy, ReplicaHostBuilder, ReplicaHostRuntime};
pub use application::{OutcomeWindow, OUTCOME_WINDOW_DEFAULT_CAPACITY};
pub use application::{ProposalCache, DEFAULT_PROPOSAL_CACHE_CAPACITY};
pub use application::{ReadConsistency, READ_CONSISTENCY_HEADER};
pub use infrastructure::AppliedIndexStore;
pub use infrastructure::PeerTransport;
pub use infrastructure::{parse_peer_overrides, replica_mode};
pub use infrastructure::{FsyncPolicy, RaftStore};
pub use interfaces::{ClusterStateView, PeerAddr, RaftRole};
pub use interfaces::{GroupRegistry, RaftRegistry, RegistryError};

// Re-export the raft_core surface a host consumer needs (membership, ids).
pub use raft_core::{
    auto_membership, DemotionRefused, Index, Membership, NodeId, PromotionRefused, RemovalRefused,
    Term, TransferRefused,
};
