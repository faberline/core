use super::*;

/// Why a leader refused a learner admission request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdmissionRefused {
    Unroutable { target: NodeId },
    NotLeaderOrTransferInFlight,
}

impl std::fmt::Display for AdmissionRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdmissionRefused::Unroutable { target } => {
                write!(f, "no address registered for peer {target}")
            }
            AdmissionRefused::NotLeaderOrTransferInFlight => {
                write!(f, "not leader or leadership transfer in flight")
            }
        }
    }
}

impl std::error::Error for AdmissionRefused {}

impl RaftHost {
    /// Access the underlying raft store.
    pub fn store(&self) -> &RaftStore {
        &self.shared.store
    }

    pub async fn is_leader(&self) -> bool {
        self.shared.node.lock().await.is_leader()
    }
    pub async fn leader(&self) -> Option<NodeId> {
        self.shared.node.lock().await.leader()
    }
    /// Hand leadership to an eligible caught-up voter before shutdown (#3664).
    pub async fn handoff_leadership(&self) -> LeadershipHandoff {
        let (outcome, transferred) = {
            let mut node = self.shared.node.lock().await;
            if !node.is_leader() {
                (LeadershipHandoff::NotLeader, false)
            } else {
                let voters = node.conf_state().membership.voters.len();
                if voters <= 1 {
                    (LeadershipHandoff::SoleVoter, false)
                } else if let Some(target) = node.handoff_candidate() {
                    match node.transfer_leadership(target) {
                        Ok(()) => (LeadershipHandoff::Transferred { target }, true),
                        Err(_) => (LeadershipHandoff::NoCaughtUpVoter { voters }, false),
                    }
                } else {
                    (LeadershipHandoff::NoCaughtUpVoter { voters }, false)
                }
            }
        };
        if transferred {
            self.shared.flush().await;
        }
        outcome
    }
    /// Transfer leadership to a named caught-up voter (#3586).
    pub async fn transfer_leadership(
        &self,
        target: NodeId,
    ) -> std::result::Result<(), TransferRefused> {
        let res = self.shared.node.lock().await.transfer_leadership(target);
        if res.is_ok() {
            self.shared.flush().await;
        }
        res
    }
    /// Promote a caught-up learner to voter (#3646).
    pub async fn promote_learner(
        &self,
        target: NodeId,
    ) -> std::result::Result<Index, PromotionRefused> {
        let res = self.shared.node.lock().await.promote_learner(target);
        if res.is_ok() {
            self.shared.flush().await;
        }
        res
    }
    /// Demote a voter to a learner (#3646).
    pub async fn demote_voter(
        &self,
        target: NodeId,
    ) -> std::result::Result<Index, DemotionRefused> {
        let res = self.shared.node.lock().await.demote_voter(target);
        if res.is_ok() {
            self.shared.flush().await;
        }
        res
    }
    /// Remove a member from the group (#3646).
    pub async fn remove_member(
        &self,
        target: NodeId,
    ) -> std::result::Result<Index, RemovalRefused> {
        let res = self.shared.node.lock().await.remove_member(target);
        if res.is_ok() {
            self.shared.flush().await;
        }
        res
    }
    /// Add or update the address of a peer (#3650).
    pub async fn upsert_peer(&self, peer: NodeId, url: String) {
        self.shared
            .peers
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(peer, url);
    }
    /// Remove the address of a peer (#3650).
    pub async fn forget_peer(&self, peer: NodeId) {
        self.shared
            .peers
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&peer);
        self.shared
            .peer_lanes
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&peer);
    }
    /// Admit a new learner to the group (#3650).
    pub async fn add_learner(
        &self,
        target: NodeId,
    ) -> std::result::Result<Index, AdmissionRefused> {
        let is_routable = self
            .shared
            .peers
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(&target);
        if !is_routable {
            return Err(AdmissionRefused::Unroutable { target });
        }
        let res = {
            let mut node = self.shared.node.lock().await;
            match node.add_learner(target) {
                Some(idx) => Ok(idx),
                None => Err(AdmissionRefused::NotLeaderOrTransferInFlight),
            }
        };
        if res.is_ok() {
            self.shared.flush().await;
        }
        res
    }
    /// Watch the state machine's applied head (followers await an index here).
    pub fn applied_watch(&self) -> watch::Receiver<Index> {
        self.shared.applied_tx.subscribe()
    }
}
