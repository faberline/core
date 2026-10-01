//! The replicated service host's startup: it builds the mTLS `PeerTransport`,
//! reads the topology from the environment, opens the `RaftStore` and spawns
//! the host, in that order.

use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Context, Result};

use crate::application::{
    ClusterTopology, HostConfig, MembershipError, MembershipPolicy, RaftHost, RaftStateMachine,
    ReplicaHostBuilder,
};
use crate::infrastructure::{FsyncPolicy, PeerTransport, RaftStore};

/// A fully started replicated host and the transport used by its peer server.
pub struct ReplicaHostRuntime {
    pub host: Arc<RaftHost>,
    pub peer_transport: PeerTransport,
    pub peer_port: u16,
    pub topology: ClusterTopology,
}

impl<P> ReplicaHostBuilder<P>
where
    P: MembershipPolicy,
{
    /// Read the standard StatefulSet topology and apply product policy.
    pub fn topology(&self) -> Result<ClusterTopology> {
        let topology = ClusterTopology::from_env_with_scheme(
            &self.service_prefix,
            &self.headless_service,
            self.peer_port,
            &self.peers_override,
            &self.scheme,
        )?;
        self.membership_policy
            .validate(&topology)
            .map_err(MembershipError::into_anyhow)?;
        Ok(topology)
    }

    /// Start a host whose peer traffic is protected by required mutual TLS.
    ///
    /// The product chooses this secure startup path. The runtime then owns the
    /// exact order and never falls back to a clear-text peer transport.
    pub fn build_secure<S>(
        &self,
        data_dir: &Path,
        state_machine: Arc<S>,
        peer_tls_env_prefix: &str,
        fsync_policy: FsyncPolicy,
        host_config: HostConfig,
    ) -> Result<ReplicaHostRuntime>
    where
        S: RaftStateMachine,
    {
        if self.scheme != "https" {
            bail!("secure replica host requires the https peer URL scheme");
        }
        let tls = peer_tls::PeerTlsConfig::from_env(peer_tls_env_prefix)
            .context("load replicated peer mTLS material")?
            .context("replicated peer mTLS material is required")?;
        if !tls.required() {
            bail!("replicated peer mTLS must be required; set {peer_tls_env_prefix}_MTLS=on");
        }
        let peer_transport =
            PeerTransport::from_config(&tls).context("build replicated peer mTLS transport")?;
        let topology = self.topology()?;
        let store_dir = data_dir
            .to_str()
            .context("raft data directory must be valid UTF-8")?;
        let store = RaftStore::open(store_dir, topology.node_id, fsync_policy)
            .context("open durable raft store")?;
        let host = Arc::new(RaftHost::spawn_with_peer_transport(
            topology.node_id,
            topology.membership.clone(),
            topology.peers.clone(),
            store,
            state_machine as Arc<dyn RaftStateMachine>,
            host_config,
            peer_transport.clone(),
        ));
        Ok(ReplicaHostRuntime {
            host,
            peer_transport,
            peer_port: self.peer_port,
            topology,
        })
    }
}
