//! The public `RaftHost` constructors: they connect the HTTP peer client
//! adapter and hand it to the host as its delivery and peer client ports.

use std::collections::HashMap;
use std::sync::Arc;

use raft_core::{Membership, NodeId};

use crate::application::{HostConfig, PeerWiring, RaftHost, RaftStateMachine};
use crate::domain::{GroupId, LEGACY_GROUP_ID};
use crate::infrastructure::{HttpPeerClient, PeerTransport, RaftStore};

impl RaftHost {
    /// Build a host for node `id`, recovering persisted state + replaying the
    /// resident committed log into the state machine, and start the tick + pump.
    /// `peers` maps the other members to base URLs (empty ⇒ single-node).
    pub fn spawn(
        id: NodeId,
        membership: Membership,
        peers: HashMap<NodeId, String>,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
        cfg: HostConfig,
    ) -> RaftHost {
        Self::spawn_group(
            id,
            GroupId(LEGACY_GROUP_ID.to_string()),
            membership,
            peers,
            store,
            sm,
            cfg,
        )
    }

    pub fn spawn_group(
        id: NodeId,
        group_id: GroupId,
        membership: Membership,
        peers: HashMap<NodeId, String>,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
        cfg: HostConfig,
    ) -> RaftHost {
        Self::spawn_with_ports(
            id,
            group_id,
            membership,
            peers,
            store,
            sm,
            cfg,
            connect(None),
        )
    }

    /// Spawn a host whose outgoing peer RPCs use the current generation of a
    /// shared mutually authenticated HTTPS transport. Callers serve
    /// [`Self::router`] on [`PeerTransport::serve`] using the same clone.
    pub fn spawn_with_peer_transport(
        id: NodeId,
        membership: Membership,
        peers: HashMap<NodeId, String>,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
        cfg: HostConfig,
        peer_transport: PeerTransport,
    ) -> RaftHost {
        Self::spawn_with_peer_transport_group(
            id,
            GroupId(LEGACY_GROUP_ID.to_string()),
            membership,
            peers,
            store,
            sm,
            cfg,
            peer_transport,
        )
    }

    pub fn spawn_with_peer_transport_group(
        id: NodeId,
        group_id: GroupId,
        membership: Membership,
        peers: HashMap<NodeId, String>,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
        cfg: HostConfig,
        peer_transport: PeerTransport,
    ) -> RaftHost {
        Self::spawn_with_ports(
            id,
            group_id,
            membership,
            peers,
            store,
            sm,
            cfg,
            connect(Some(peer_transport)),
        )
    }
}

/// Connect the HTTP peer client, sending over `peer_transport` when given and
/// over its own h2c client otherwise.
fn connect(
    peer_transport: Option<PeerTransport>,
) -> impl FnOnce(PeerWiring) -> Arc<HttpPeerClient> {
    move |wiring| {
        Arc::new(HttpPeerClient::connect(
            wiring.id,
            wiring.group_id,
            wiring.peers,
            peer_transport,
            wiring.rpc_timeout,
            wiring.propose_timeout,
            wiring.snapshot_rpc_timeout,
        ))
    }
}
