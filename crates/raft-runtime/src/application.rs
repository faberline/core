//! Application layer: the raft host (`RaftHost` and its tick, apply, propose,
//! membership, snapshot and shutdown flows), its configuration, the multi-group
//! registry, the cluster-topology math and the state-machine contract a service
//! supplies.

mod config;
mod fenced_assignment;
mod host;
mod outcome_window;
mod port_error;
mod proposal_cache;
mod read_consistency;
mod registry;
mod replica_host;
mod state_machine;
mod topology;

pub use config::{HostConfig, SnapshotPolicy};
pub use fenced_assignment::{
    ActiveAssignment, AssignmentEpoch, AssignmentError, FenceToken, FencedAssignment,
};
pub use host::{
    AdmissionRefused, ChunkSink, HostShutdownReport, LeadershipHandoff, MembershipPhase,
    PhaseRecord, PhaseStatus, ProposalBackpressure, ProposalOutcome, RaftHost, RaftStatus,
    ShutdownCaller, ShutdownPhase, SnapshotCompactionOutcome, StorageFailed, SNAPSHOT_CHUNK_SIZE,
};
pub use outcome_window::{OutcomeWindow, DEFAULT_CAPACITY as OUTCOME_WINDOW_DEFAULT_CAPACITY};
pub use port_error::{MembershipError, StateMachineError};
pub use proposal_cache::{ProposalCache, DEFAULT_PROPOSAL_CACHE_CAPACITY};
pub use read_consistency::{ReadConsistency, READ_CONSISTENCY_HEADER};
pub use registry::{GroupRegistry, RaftRegistry, RegistryError};
pub use replica_host::{MembershipPolicy, ReplicaHostBuilder, ReplicaHostRuntime};
pub use state_machine::{
    AdmissionPermit, Command, PreparedSnapshot, RaftStateMachine, SnapshotPreparation,
};
pub use topology::{peer_ordinal, ClusterDims, ClusterTopology};

pub(crate) use host::{
    apply_ready, cold_start, decode_backpressure, persist_node, take_reply, PeerLaneQueue,
    PeerWiring, Shared,
};
pub(crate) use topology::check_peer_scheme;
