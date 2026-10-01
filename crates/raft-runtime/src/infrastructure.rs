//! Infrastructure layer: durable raft storage, the HTTP peer client adapter
//! and its wire envelopes, the mTLS peer transport, the applied-index file and
//! the environment-derived cluster topology.

mod applied_index_store;
mod peer_rpc;
mod peer_transport;
pub(crate) mod peer_wire;
mod store;
mod topology_env;

pub use applied_index_store::AppliedIndexStore;
pub use peer_transport::PeerTransport;
pub use store::{FsyncPolicy, RaftStore};
pub use topology_env::{parse_peer_overrides, replica_mode};

pub(crate) use peer_rpc::HttpPeerClient;
