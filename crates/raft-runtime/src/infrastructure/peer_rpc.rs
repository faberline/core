//! The outbound half of the peer transport: `HttpPeerClient` sends Vote /
//! Append / InstallSnapshot / TimeoutNow requests, `/raftz` status reads and
//! forwarded proposals to other members over h2c or the mTLS transport.

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::Duration;

use raft_core::{
    AppendResp, Index, InstallSnapshotReq, InstallSnapshotResp, NodeId, RaftDelivery, RaftMsg,
    Term, VoteResp,
};
use serde::Serialize;

use super::peer_wire::{
    AppendEnvelope, CapableSnapEnvelope, CapableSnapshotResp, SnapEnvelope, TimeoutNowEnvelope,
    VoteEnvelope,
};
use super::PeerTransport;
use crate::domain::{ForwardReply, GroupId, PeerClient};

mod forward;

/// The HTTP peer client of one raft group member: its peer address book, its
/// h2c client (or the shared mTLS transport) and the counters and nonces the
/// outbound RPCs need.
pub(crate) struct HttpPeerClient {
    id: NodeId,
    group_id: GroupId,
    peers: RwLock<HashMap<NodeId, String>>,
    client: reqwest::Client,
    peer_transport: Option<PeerTransport>,
    rpc_timeout: Duration,
    propose_timeout: Duration,
    snapshot_rpc_timeout: Duration,
    snapshot_nonce: AtomicU64,
    undeliverable_withdrawn_address: AtomicU64,
}

impl HttpPeerClient {
    /// A client for member `id` of `group_id`. Without `peer_transport` it
    /// sends over its own h2c client, whose request timeout is `rpc_timeout`.
    pub(crate) fn connect(
        id: NodeId,
        group_id: GroupId,
        peers: HashMap<NodeId, String>,
        peer_transport: Option<PeerTransport>,
        rpc_timeout: Duration,
        propose_timeout: Duration,
        snapshot_rpc_timeout: Duration,
    ) -> Self {
        let client = transport_h2c::h2c_client_with(Some(rpc_timeout), None).expect("h2c client");
        Self {
            id,
            group_id,
            peers: RwLock::new(peers),
            client,
            peer_transport,
            rpc_timeout,
            propose_timeout,
            snapshot_rpc_timeout,
            snapshot_nonce: AtomicU64::new(1),
            undeliverable_withdrawn_address: AtomicU64::new(0),
        }
    }

    fn http_client(&self) -> reqwest::Client {
        self.peer_transport
            .as_ref()
            .map(PeerTransport::http_client)
            .unwrap_or_else(|| self.client.clone())
    }

    fn base(&self, to: NodeId) -> io::Result<String> {
        self.address(&to).ok_or_else(|| {
            io::Error::other(format!("raft: voter {to} has no registered peer address"))
        })
    }

    async fn post<T: Serialize>(&self, url: &str, body: &T) -> Option<Vec<u8>> {
        self.post_with_timeout(url, body, self.rpc_timeout).await
    }

    async fn post_with_timeout<T: Serialize>(
        &self,
        url: &str,
        body: &T,
        timeout: Duration,
    ) -> Option<Vec<u8>> {
        match self
            .http_client()
            .post(url)
            .timeout(timeout)
            .json(body)
            .send()
            .await
        {
            Ok(r) => r.bytes().await.ok().map(|b| b.to_vec()),
            Err(_) => None,
        }
    }

    async fn send(&self, to: NodeId, msg: RaftMsg) -> Option<RaftMsg> {
        let Some(base) = self.address(&to) else {
            self.count_withdrawn(to);
            return None;
        };
        match msg {
            RaftMsg::Vote(req) => self
                .post(
                    &format!("{base}/raft/request-vote"),
                    &VoteEnvelope {
                        group_id: self.group_id.as_str().to_owned(),
                        from: self.id,
                        req,
                    },
                )
                .await
                .and_then(|r| serde_json::from_slice::<VoteResp>(&r).ok())
                .map(RaftMsg::VoteResp),
            RaftMsg::Append(req) => self
                .post(
                    &format!("{base}/raft/append-entries"),
                    &AppendEnvelope {
                        group_id: self.group_id.as_str().to_owned(),
                        from: self.id,
                        req,
                    },
                )
                .await
                .and_then(|r| serde_json::from_slice::<AppendResp>(&r).ok())
                .map(RaftMsg::AppendResp),
            RaftMsg::InstallSnapshot(req) => self
                .request_snapshot(to, req, None)
                .await
                .ok()
                .map(RaftMsg::InstallSnapshotResp),
            RaftMsg::TimeoutNow(req) => {
                self.post(
                    &format!("{base}/raft/timeout-now"),
                    &TimeoutNowEnvelope {
                        group_id: self.group_id.as_str().to_owned(),
                        from: self.id,
                        req,
                    },
                )
                .await;
                None
            }
            _ => None,
        }
    }

    async fn request_snapshot(
        &self,
        to: NodeId,
        req: InstallSnapshotReq,
        required_capability: Option<&'static str>,
    ) -> io::Result<InstallSnapshotResp> {
        let base = self.base(to)?;
        if let Some(required) = required_capability {
            let nonce = self.snapshot_nonce.fetch_add(1, Ordering::AcqRel);
            let bytes = self
                .post_with_timeout(
                    &format!("{base}/raft/install-snapshot-capable"),
                    &CapableSnapEnvelope {
                        group_id: self.group_id.as_str().to_owned(),
                        from: self.id,
                        req,
                        snapshot_capability: required.to_string(),
                        snapshot_nonce: nonce,
                    },
                    self.snapshot_rpc_timeout,
                )
                .await
                .ok_or_else(|| {
                    io::Error::other(format!(
                        "raft: voter {to} did not answer capable snapshot install"
                    ))
                })?;
            let response: CapableSnapshotResp =
                serde_json::from_slice(&bytes).map_err(|error| {
                    io::Error::other(format!(
                        "raft: voter {to} returned an invalid capable snapshot reply: {error}"
                    ))
                })?;
            if response.snapshot_capability != required || response.snapshot_nonce != nonce {
                return Err(io::Error::other(format!(
                    "raft: voter {to} did not echo snapshot capability {required}"
                )));
            }
            return Ok(InstallSnapshotResp {
                term: Term::new(response.term),
                accepted: response.accepted,
                snapshot_index: Index::new(response.snapshot_index),
            });
        }
        let bytes = self
            .post_with_timeout(
                &format!("{base}/raft/install-snapshot"),
                &SnapEnvelope {
                    group_id: self.group_id.as_str().to_owned(),
                    from: self.id,
                    req,
                },
                self.snapshot_rpc_timeout,
            )
            .await
            .ok_or_else(|| {
                io::Error::other(format!("raft: voter {to} did not answer snapshot install"))
            })?;
        serde_json::from_slice(&bytes).map_err(|error| {
            io::Error::other(format!(
                "raft: voter {to} returned an invalid snapshot reply: {error}"
            ))
        })
    }

    async fn request_status(&self, to: NodeId) -> io::Result<Vec<u8>> {
        let base = self.base(to)?;
        match self
            .http_client()
            .get(format!("{base}/raftz"))
            .timeout(self.rpc_timeout)
            .send()
            .await
        {
            Ok(response) => response.bytes().await.map(|b| b.to_vec()).map_err(|error| {
                io::Error::other(format!(
                    "raft: voter {to} returned an invalid status: {error}"
                ))
            }),
            Err(error) => Err(io::Error::other(format!(
                "raft: voter {to} did not answer status: {error}"
            ))),
        }
    }

    fn count_withdrawn(&self, to: NodeId) {
        self.undeliverable_withdrawn_address
            .fetch_add(1, Ordering::Relaxed);
        tracing::warn!(
            target = to.get(),
            group = %self.group_id,
            "raft: discarded in-flight message to withdrawn peer address"
        );
    }
}

impl RaftDelivery for HttpPeerClient {
    fn deliver(
        &self,
        to: NodeId,
        msg: RaftMsg,
    ) -> Pin<Box<dyn Future<Output = Option<RaftMsg>> + Send + '_>> {
        Box::pin(self.send(to, msg))
    }

    fn install_snapshot(
        &self,
        to: NodeId,
        req: InstallSnapshotReq,
        required_capability: Option<&'static str>,
    ) -> Pin<Box<dyn Future<Output = io::Result<InstallSnapshotResp>> + Send + '_>> {
        Box::pin(self.request_snapshot(to, req, required_capability))
    }
}

impl PeerClient<NodeId> for HttpPeerClient {
    fn address(&self, peer: &NodeId) -> Option<String> {
        self.peers
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(peer)
            .cloned()
    }

    fn set_address(&self, peer: NodeId, url: String) {
        self.peers
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(peer, url);
    }

    fn remove_address(&self, peer: &NodeId) {
        self.peers
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .remove(peer);
    }

    fn discard_if_withdrawn(&self, peer: &NodeId) -> bool {
        if self.address(peer).is_some() {
            return false;
        }
        self.count_withdrawn(*peer);
        true
    }

    fn withdrawn_address_drops(&self) -> u64 {
        self.undeliverable_withdrawn_address.load(Ordering::Relaxed)
    }

    fn read_status(
        &self,
        peer: NodeId,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<u8>>> + Send + '_>> {
        Box::pin(self.request_status(peer))
    }

    fn forward<'a>(
        &'a self,
        leader_url: &'a str,
        command: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = ForwardReply> + Send + 'a>> {
        Box::pin(self.forward_to_leader(leader_url, command))
    }
}
