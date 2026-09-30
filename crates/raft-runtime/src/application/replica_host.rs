//! Shared construction for one replicated service host.
//!
//! The runtime owns the startup order. A service supplies only its membership
//! policy, state machine, storage location, and service-specific names. The
//! builder's `topology` and `build_secure` live in the composition root
//! (`src/app/replica_host.rs`): they read the environment and pick the store
//! and the peer transport.

use anyhow::{bail, Result};

use super::{ClusterTopology, MembershipError};

/// Product policy applied after the shared topology has been read and checked.
pub trait MembershipPolicy: Send + Sync + 'static {
    fn validate(&self, topology: &ClusterTopology) -> std::result::Result<(), MembershipError>;
}

/// Builds the common topology, peer transport, durable store, and Raft host.
pub struct ReplicaHostBuilder<P> {
    pub(crate) service_prefix: String,
    pub(crate) headless_service: String,
    pub(crate) peer_port: u16,
    pub(crate) peers_override: String,
    pub(crate) scheme: String,
    pub(crate) membership_policy: P,
}

impl<P> ReplicaHostBuilder<P>
where
    P: MembershipPolicy,
{
    pub fn new(
        service_prefix: impl Into<String>,
        headless_service: impl Into<String>,
        peer_port: u16,
        peers_override: impl Into<String>,
        scheme: impl Into<String>,
        membership_policy: P,
    ) -> Result<Self> {
        let service_prefix = service_prefix.into();
        let headless_service = headless_service.into();
        let peers_override = peers_override.into();
        let scheme = scheme.into();
        if service_prefix.trim().is_empty() {
            bail!("raft service prefix must not be empty");
        }
        if headless_service.trim().is_empty() {
            bail!("raft headless service must not be empty");
        }
        if peers_override.trim().is_empty() {
            bail!("raft peer override environment variable must not be empty");
        }
        if peer_port == 0 {
            bail!("raft peer port must be greater than zero");
        }
        if !matches!(scheme.as_str(), "http" | "https") {
            bail!("raft peer URL scheme must be http or https");
        }
        Ok(Self {
            service_prefix,
            headless_service,
            peer_port,
            peers_override,
            scheme,
            membership_policy,
        })
    }
}
