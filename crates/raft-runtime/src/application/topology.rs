//! k8s-native cluster topology + auto-mode for the raft host.
//!
//! Every raft_core service derives the same thing from the StatefulSet downward
//! API: which mode to run (single-node vs replica/HA), this node's id, the
//! group membership, and the peer URLs. This module centralizes it so services
//! compose it instead of hand-rolling the ordinal math + peer-DNS each time.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use crate::{Membership, NodeId};

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

/// Refuse any peer URL scheme but `http` and `https`, so a malformed scheme
/// cannot silently weaken or redirect peer traffic.
pub(crate) fn check_peer_scheme(scheme: &str) -> Result<()> {
    if !matches!(scheme, "http" | "https") {
        bail!("raft peer URL scheme must be http or https");
    }
    Ok(())
}

impl ClusterTopology {
    /// Validate `dims` and derive this node's id, the membership and the peer
    /// URLs. `overrides` replaces the peer-DNS address of the replica at the
    /// same position (`host` or `host:port`).
    pub(crate) fn from_dims(
        dims: &ClusterDims,
        fallback_prefix: &str,
        headless_service: &str,
        peer_port: u16,
        overrides: &[String],
        scheme: &str,
    ) -> Result<Self> {
        let shard_count = dims.shard_count;
        let replicas_per_shard = dims.replicas_per_shard;
        let voter_count = dims.voter_count;
        if shard_count == 0 {
            bail!("SHARD_COUNT must be greater than zero");
        }
        if replicas_per_shard == 0 {
            bail!("REPLICAS_PER_SHARD must be greater than zero");
        }
        if voter_count == 0 || voter_count > replicas_per_shard {
            bail!("VOTER_COUNT must be in 1..=REPLICAS_PER_SHARD");
        }
        let shard_index = dims.shard_index()?;
        let node_id = dims.replica_index()? as NodeId;
        if node_id >= replicas_per_shard as NodeId {
            bail!("POD_NAME ordinal resolves outside REPLICAS_PER_SHARD");
        }

        // The peer-DNS prefix is this pod's own StatefulSet name. Only fall back
        // to the caller's guess if `POD_NAME` somehow carries none — and say so,
        // because at that point peer addressing rests on an assumption rather
        // than on observed identity.
        let prefix = match dims.pod_prefix() {
            Ok(p) => p,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    fallback = fallback_prefix,
                    "raft peer prefix not derivable from POD_NAME; falling back to the caller's"
                );
                fallback_prefix
            }
        };
        if prefix != fallback_prefix {
            // Not a problem — it is the normal case for any CR not named after
            // the binary — but it is the one fact that makes peer URLs resolve,
            // so it belongs in the startup log next to node_id and peers.
            tracing::debug!(
                derived = prefix,
                caller_default = fallback_prefix,
                "raft peer prefix taken from POD_NAME's StatefulSet name"
            );
        }

        // pod ordinal → (shard, replica) is pure integer math, so peers are found
        // via headless DNS with no discovery service. `index N → replica N`.
        let mut peers = HashMap::new();
        for replica in 0..replicas_per_shard {
            let id = replica as NodeId;
            if id == node_id {
                continue;
            }
            let url = match overrides.get(replica as usize) {
                Some(addr) if addr.contains(':') => format!("{scheme}://{addr}"),
                Some(addr) => format!("{scheme}://{addr}:{peer_port}"),
                None => {
                    let ordinal = peer_ordinal(shard_count, shard_index, replica);
                    format!("{scheme}://{prefix}-{ordinal}.{headless_service}:{peer_port}")
                }
            };
            peers.insert(id, url);
        }

        let membership = Membership::new(
            (0..voter_count as NodeId).collect(),
            (voter_count as NodeId..replicas_per_shard as NodeId).collect(),
        );
        Ok(Self {
            node_id,
            membership,
            peers,
            replicas_per_shard,
            shard_index,
        })
    }
}

#[cfg(test)]
mod tests;
