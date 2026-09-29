//! k8s-native cluster topology + auto-mode for the raft host.
//!
//! Every raft_core service derives the same thing from the StatefulSet downward
//! API: which mode to run (single-node vs replica/HA), this node's id, the
//! group membership, and the peer URLs. This module centralizes it so services
//! compose it instead of hand-rolling the ordinal math + peer-DNS each time.

pub use crate::application::{
    ensure_static_membership_unchanged, peer_ordinal, ClusterDims, ClusterTopology,
};
pub use crate::infrastructure::{parse_peer_overrides, replica_mode};
