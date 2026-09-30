//! The cluster topology read from the process environment: the downward-API
//! quartet and the peer-address override list.

use anyhow::{Context, Result};

use crate::application::{check_peer_scheme, ClusterDims, ClusterTopology};
use crate::infrastructure::parse_peer_overrides;

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
        check_peer_scheme(scheme)?;
        let dims = ClusterDims::from_env()?;
        let overrides = parse_peer_overrides(peers_override);
        Self::from_dims(
            &dims,
            fallback_prefix,
            headless_service,
            peer_port,
            &overrides,
            scheme,
        )
    }
}

fn parse_env(key: &str) -> Result<u32> {
    std::env::var(key)
        .with_context(|| format!("{key} not set"))?
        .parse()
        .with_context(|| format!("{key} must be a u32"))
}
