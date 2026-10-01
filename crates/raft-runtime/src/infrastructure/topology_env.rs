//! Environment adapters for the cluster topology: the replica-mode switch,
//! the downward-API quartet, peer-address overrides and the peer-DNS URLs.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use crate::application::{peer_ordinal, ClusterDims, ClusterTopology};
use crate::{Membership, NodeId};

/// Whether the StatefulSet runs in replica/HA mode: `true` when
/// `REPLICAS_PER_SHARD > 1`. A single replica — or no cluster context (the env
/// unset, e.g. local dev) — is single-node. This is the **auto-mode** switch: a
/// service defaults to single-node and turns on raft only when k8s scales it out.
pub fn replica_mode() -> bool {
    std::env::var("REPLICAS_PER_SHARD")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1)
        > 1
}

impl ClusterDims {
    /// Read the standard downward-API quartet.
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            shard_count: parse_env("SHARD_COUNT")?,
            replicas_per_shard: parse_env("REPLICAS_PER_SHARD")?,
            voter_count: parse_env("VOTER_COUNT")?,
            pod_name: std::env::var("POD_NAME").context("POD_NAME not set")?,
        })
    }
}

/// Parse a `LUMEN_PEERS`-style override env var (`host[:port],host[:port],...`,
/// empty entries filtered) into an `index -> host[:port]` override list.
/// Empty when `env_var` is unset — callers then use the DNS-derived
/// addresses unmodified. Shared by [`ClusterTopology::from_env`] and any
/// caller enumerating its own peer records with the same override contract.
pub fn parse_peer_overrides(env_var: &str) -> Vec<String> {
    std::env::var(env_var)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

impl ClusterTopology {
    /// Build from the standard downward-API env (`POD_NAME`, `SHARD_COUNT`,
    /// `REPLICAS_PER_SHARD`, `VOTER_COUNT`) and a peer-DNS template
    /// (`<prefix>-<ordinal>.<headless_service>:<peer_port>`). `peers_override` is
    /// the name of an env var (e.g. `LUMEN_PEERS`) holding `host[:port],...` that
    /// replaces the DNS addresses — for running a multi-node group on one machine.
    ///
    /// `fallback_prefix` is only consulted when `POD_NAME` carries no usable
    /// StatefulSet name; see [`Self::from_env_with_scheme`] for why the pod's own
    /// name wins.
    pub fn from_env(
        fallback_prefix: &str,
        headless_service: &str,
        peer_port: u16,
        peers_override: &str,
    ) -> Result<Self> {
        Self::from_env_with_scheme(
            fallback_prefix,
            headless_service,
            peer_port,
            peers_override,
            "http",
        )
    }

    /// TLS-aware variant used by stateful services serving Raft on a dedicated
    /// mTLS peer port. Only `http` and `https` are accepted so a malformed
    /// scheme cannot silently weaken or redirect peer traffic.
    ///
    /// The peer-DNS prefix comes from [`ClusterDims::pod_prefix`] — the pod's
    /// own StatefulSet name — and `fallback_prefix` is used only if `POD_NAME`
    /// carries none. Callers used to pass their binary name here as the real
    /// prefix, which is right only when the custom resource happens to be named
    /// after the binary: an operator names the StatefulSet after the CR, so a CR
    /// named `quorum` produces pods `quorum-0`/`quorum-1` while the binary was
    /// addressing `lumen-1.<headless>`. That name does not resolve, no
    /// `RequestVote` is ever delivered, and every voter campaigns forever
    /// without ever hearing a peer — a silent, permanent loss of quorum whose
    /// only symptom is that no leader appears. Deriving the prefix from
    /// `POD_NAME` makes it correct for any CR name, and identical to the old
    /// behavior when the two already agreed.
    pub fn from_env_with_scheme(
        fallback_prefix: &str,
        headless_service: &str,
        peer_port: u16,
        peers_override: &str,
        scheme: &str,
    ) -> Result<Self> {
        if !matches!(scheme, "http" | "https") {
            bail!("raft peer URL scheme must be http or https");
        }
        let dims = ClusterDims::from_env()?;
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
        let overrides = parse_peer_overrides(peers_override);

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

        let membership = Membership {
            voters: (0..voter_count as NodeId).collect(),
            learners: (voter_count as NodeId..replicas_per_shard as NodeId).collect(),
        };
        Ok(Self {
            node_id,
            membership,
            peers,
            replicas_per_shard,
            shard_index,
        })
    }
}

fn parse_env(key: &str) -> Result<u32> {
    std::env::var(key)
        .with_context(|| format!("{key} not set"))?
        .parse()
        .with_context(|| format!("{key} must be a u32"))
}

#[cfg(test)]
mod tests;
