//! The host's calls to other members, made through the delivery and peer
//! client ports.

use super::*;

impl Shared {
    /// Deliver one lane message to `to` and feed any reply back into the node.
    pub(crate) async fn send_request(self: Arc<Self>, to: NodeId, msg: RaftMsg) {
        let reply = match msg {
            RaftMsg::InstallSnapshot(req) => {
                if self.peer_client.discard_if_withdrawn(&to) {
                    return;
                }
                self.request_snapshot(to, req, self.sm.snapshot_capability())
                    .await
                    .ok()
                    .map(RaftMsg::InstallSnapshotResp)
            }
            msg => self.delivery.deliver(to, msg).await,
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

    pub(crate) async fn request_snapshot(
        &self,
        to: NodeId,
        req: InstallSnapshotReq,
        required_capability: Option<&'static str>,
    ) -> Result<InstallSnapshotResp> {
        Ok(self
            .delivery
            .install_snapshot(to, req, required_capability)
            .await?)
    }

    pub(crate) async fn request_status(&self, to: NodeId) -> Result<RaftStatus> {
        let bytes = self.peer_client.read_status(to).await?;
        serde_json::from_slice(&bytes)
            .map_err(|error| anyhow!("raft: voter {to} returned an invalid status: {error}"))
    }

    /// Lane messages discarded because their peer's address was withdrawn.
    pub(crate) fn undeliverable_withdrawn_address(&self) -> u64 {
        self.peer_client.withdrawn_address_drops()
    }
}
