//! The outbound half of the peer transport: Vote / Append / InstallSnapshot /
//! TimeoutNow requests and `/raftz` status reads sent to other members.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use raft_core::{AppendResp, InstallSnapshotReq, InstallSnapshotResp, NodeId, RaftMsg, VoteResp};
use serde::Serialize;

use super::peer_wire::{
    AppendEnvelope, CapableSnapEnvelope, CapableSnapshotResp, SnapEnvelope, TimeoutNowEnvelope,
    VoteEnvelope,
};
use crate::application::{RaftStatus, Shared};

impl Shared {
    pub(crate) async fn send_request(self: Arc<Self>, to: NodeId, msg: RaftMsg) {
        let Some(base) = self
            .peers
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(&to)
            .cloned()
        else {
            self.undeliverable_withdrawn_address
                .fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                target = to,
                group = %self.group_id.0,
                "raft: discarded in-flight message to withdrawn peer address"
            );
            return;
        };
        let reply: Option<RaftMsg> = match msg {
            RaftMsg::Vote(req) => self
                .post(
                    &format!("{base}/raft/request-vote"),
                    &VoteEnvelope {
                        group_id: self.group_id.0.clone(),
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
                        group_id: self.group_id.0.clone(),
                        from: self.id,
                        req,
                    },
                )
                .await
                .and_then(|r| serde_json::from_slice::<AppendResp>(&r).ok())
                .map(RaftMsg::AppendResp),
            RaftMsg::InstallSnapshot(req) => self
                .request_snapshot(to, req, self.sm.snapshot_capability())
                .await
                .ok()
                .map(RaftMsg::InstallSnapshotResp),
            RaftMsg::TimeoutNow(req) => {
                self.post(
                    &format!("{base}/raft/timeout-now"),
                    &TimeoutNowEnvelope {
                        group_id: self.group_id.0.clone(),
                        from: self.id,
                        req,
                    },
                )
                .await;
                None
            }
            _ => None,
        };
        if let Some(reply) = reply {
            let mut n = self.node.lock().await;
            n.handle(to, reply);
            if self.persist(&n).is_err() {
                return;
            }
            self.apply_ready(&mut n);
            // Subsequent outbound work is shipped by the pump (no recursive flush).
        }
    }

    async fn post<T: Serialize>(&self, url: &str, body: &T) -> Option<Vec<u8>> {
        self.post_with_timeout(url, body, self.cfg.rpc_timeout)
            .await
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

    pub(crate) async fn request_snapshot(
        &self,
        to: NodeId,
        req: InstallSnapshotReq,
        required_capability: Option<&'static str>,
    ) -> Result<InstallSnapshotResp> {
        let base = self
            .peers
            .read()
            .unwrap_or_else(|peers| peers.into_inner())
            .get(&to)
            .cloned()
            .ok_or_else(|| anyhow!("raft: voter {to} has no registered peer address"))?;
        if let Some(required) = required_capability {
            let nonce = self.snapshot_nonce.fetch_add(1, Ordering::AcqRel);
            let bytes = self
                .post_with_timeout(
                    &format!("{base}/raft/install-snapshot-capable"),
                    &CapableSnapEnvelope {
                        group_id: self.group_id.0.clone(),
                        from: self.id,
                        req,
                        snapshot_capability: required.to_string(),
                        snapshot_nonce: nonce,
                    },
                    self.snapshot_rpc_timeout,
                )
                .await
                .ok_or_else(|| {
                    anyhow!("raft: voter {to} did not answer capable snapshot install")
                })?;
            let response: CapableSnapshotResp =
                serde_json::from_slice(&bytes).map_err(|error| {
                    anyhow!("raft: voter {to} returned an invalid capable snapshot reply: {error}")
                })?;
            if response.snapshot_capability != required || response.snapshot_nonce != nonce {
                return Err(anyhow!(
                    "raft: voter {to} did not echo snapshot capability {required}"
                ));
            }
            return Ok(InstallSnapshotResp {
                term: response.term,
                accepted: response.accepted,
                snapshot_index: response.snapshot_index,
            });
        }
        let bytes = self
            .post_with_timeout(
                &format!("{base}/raft/install-snapshot"),
                &SnapEnvelope {
                    group_id: self.group_id.0.clone(),
                    from: self.id,
                    req,
                },
                self.snapshot_rpc_timeout,
            )
            .await
            .ok_or_else(|| anyhow!("raft: voter {to} did not answer snapshot install"))?;
        serde_json::from_slice(&bytes).map_err(|error| {
            anyhow!("raft: voter {to} returned an invalid snapshot reply: {error}")
        })
    }

    pub(crate) async fn request_status(&self, to: NodeId) -> Result<RaftStatus> {
        let base = self
            .peers
            .read()
            .unwrap_or_else(|peers| peers.into_inner())
            .get(&to)
            .cloned()
            .ok_or_else(|| anyhow!("raft: voter {to} has no registered peer address"))?;
        let bytes = match self
            .http_client()
            .get(format!("{base}/raftz"))
            .timeout(self.cfg.rpc_timeout)
            .send()
            .await
        {
            Ok(response) => response
                .bytes()
                .await
                .map_err(|error| anyhow!("raft: voter {to} returned an invalid status: {error}"))?,
            Err(error) => return Err(anyhow!("raft: voter {to} did not answer status: {error}")),
        };
        serde_json::from_slice(&bytes)
            .map_err(|error| anyhow!("raft: voter {to} returned an invalid status: {error}"))
    }
}
