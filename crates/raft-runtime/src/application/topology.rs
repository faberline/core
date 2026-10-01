//! k8s-native cluster topology + auto-mode for the raft host.
//!
//! Every raft_core service derives the same thing from the StatefulSet downward
//! API: which mode to run (single-node vs replica/HA), this node's id, the
//! group membership, and the peer URLs. This module centralizes it so services
//! compose it instead of hand-rolling the ordinal math + peer-DNS each time.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use crate::{Membership, NodeId};

/// Guard a controller that is still backed by raft-runtime's startup-static
/// membership. Changing the StatefulSet replica count without a replicated
/// membership transition is unsafe: existing members and new pods would run
/// with different quorum sets. Callers must keep the replica layer unchanged
/// until raft-core/raft-runtime expose the joint-consensus workflow.
pub fn ensure_static_membership_unchanged(current: u32, desired: u32) -> Result<()> {
    if current != desired {
        bail!(
            "unsafe replica transition {current}->{desired}: raft-runtime membership is static; complete a replicated membership transition before changing StatefulSet replicas"
        );
    }
    Ok(())
}

/// The scalar shard/replica/voter derivation from the standard downward-API
/// quartet (`SHARD_COUNT`, `REPLICAS_PER_SHARD`, `VOTER_COUNT`, `POD_NAME`) —
/// the piece [`ClusterTopology::from_env`] shares with a caller that only
/// needs the scalars, not peer URLs (e.g. lumen's `ClusterConfig`, which
/// stays compiled outside the `raft-wal` feature; #1002).
#[derive(Debug, Clone)]
pub struct ClusterDims {
    pub shard_count: u32,
    pub replicas_per_shard: u32,
    pub voter_count: u32,
    pub pod_name: String,
}

impl ClusterDims {
    /// The trailing `-<N>` ordinal in `pod_name` — the StatefulSet identity.
    pub fn pod_ordinal(&self) -> Result<u32> {
        let (_, suffix) = self
            .pod_name
            .rsplit_once('-')
            .context("POD_NAME has no '-<ordinal>' suffix")?;
        suffix
            .parse()
            .with_context(|| format!("POD_NAME ordinal '{suffix}' is not a u32"))
    }

    /// The part of `pod_name` before the ordinal — i.e. the StatefulSet's own
    /// name, which is also the peer-DNS prefix (`<sts>-<n>.<headless>`).
    ///
    /// This is the same `rsplit_once` that [`Self::pod_ordinal`] uses, keeping
    /// the other half instead of discarding it. That matters: a pod trusts the
    /// ordinal from this string to decide *who it is*, so the prefix from the
    /// identical parse is exactly as trustworthy for deciding *who to call*.
    /// A caller cannot know this value — an operator names the StatefulSet
    /// after the custom resource, so only the pod's own downward-API `POD_NAME`
    /// carries it.
    pub fn pod_prefix(&self) -> Result<&str> {
        let (prefix, _) = self
            .pod_name
            .rsplit_once('-')
            .context("POD_NAME has no '-<ordinal>' suffix")?;
        if prefix.is_empty() {
            bail!(
                "POD_NAME '{}' has an empty StatefulSet prefix",
                self.pod_name
            );
        }
        Ok(prefix)
    }

    /// `ordinal % shard_count` — which shard this pod belongs to.
    pub fn shard_index(&self) -> Result<u32> {
        Ok(self.pod_ordinal()? % self.shard_count)
    }

    /// `ordinal / shard_count` — this pod's replica index within its shard
    /// (== the raft node id).
    pub fn replica_index(&self) -> Result<u32> {
        Ok(self.pod_ordinal()? / self.shard_count)
    }

    /// Whether this replica votes (`replica_index < voter_count`); the rest
    /// are learners.
    pub fn is_voter(&self) -> Result<bool> {
        Ok(self.replica_index()? < self.voter_count)
    }
}

/// The StatefulSet pod ordinal for `replica` within a shard
/// (`replica * shard_count + shard_index`) — the peer-DNS math shared by
/// every peer enumeration: [`ClusterTopology::from_env`]'s peer URLs and a
/// caller's own richer per-peer record (e.g. lumen's `RaftGroup`/`PeerAddr`).
pub fn peer_ordinal(shard_count: u32, shard_index: u32, replica: u32) -> u32 {
    replica * shard_count + shard_index
}

/// One raft group's topology, derived from the StatefulSet downward API.
#[derive(Debug, Clone)]
pub struct ClusterTopology {
    /// This node's id within its shard's group = the replica index.
    pub node_id: NodeId,
    /// Voters `0..voter_count`, learners the rest.
    pub membership: Membership,
    /// Peer base URLs (`NodeId → http://host:port`), excluding self.
    pub peers: HashMap<NodeId, String>,
    pub replicas_per_shard: u32,
    pub shard_index: u32,
}

#[cfg(test)]
mod tests;
