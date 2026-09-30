//! The Raft consensus domain: ids, log entries and durable state, the wire
//! messages, membership and its configuration state, and the step-driven
//! [`RaftNode`] state machine.

mod conf_state;
mod entry;
mod ids;
mod membership;
mod message;
mod persisted_state;
mod raft_node;
mod refusal;
mod role;
mod timing;

pub use conf_state::ConfState;
pub use entry::{EntryKind, RaftEntry};
pub use ids::{Index, NodeId, Term};
pub use membership::{auto_membership, Membership};
pub use message::{
    AppendReq, AppendResp, InstallSnapshotReq, InstallSnapshotResp, Outgoing, RaftMsg,
    TimeoutNowReq, VoteReq, VoteResp,
};
pub use persisted_state::{PersistedState, PersistedStateRef};
pub use raft_node::RaftNode;
pub use refusal::{DemotionRefused, PromotionRefused, RemovalRefused, TransferRefused};
pub use role::Role;
pub use timing::{ELECTION_TIMEOUT_FLOOR_TICKS, HEARTBEAT_INTERVAL_TICKS};
