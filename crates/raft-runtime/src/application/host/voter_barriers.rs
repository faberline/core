use super::*;

impl RaftHost {
    /// Verify that every voter advertises the local state machine's snapshot
    /// capability before a product performs a snapshot-dependent mutation.
    pub async fn require_snapshot_capability_on_all_voters(&self) -> Result<()> {
        let _operation = self.shared.begin_coordinated_peer_work()?;
        let (term, voters) = {
            let node = self.shared.node.lock().await;
            if !node.is_leader() && node.conf_state().membership.voters.len() > 1 {
                return Err(anyhow!(
                    "only the Raft leader can verify voter snapshot capabilities"
                ));
            }
            (
                node.current_term(),
                node.conf_state().membership.voters.clone(),
            )
        };
        let required = self
            .shared
            .sm
            .snapshot_capability()
            .ok_or_else(|| anyhow!("state machine does not advertise a snapshot capability"))?;
        for voter in voters
            .iter()
            .copied()
            .filter(|voter| *voter != self.shared.id)
        {
            let status = self.shared.request_status(voter).await?;
            if status.snapshot_capability.as_deref() != Some(required) {
                return Err(anyhow!(
                    "raft: voter {voter} does not advertise snapshot capability {required}"
                ));
            }
        }
        let node = self.shared.node.lock().await;
        if node.current_term() != term
            || (!node.is_leader() && node.conf_state().membership.voters.len() > 1)
        {
            return Err(anyhow!(
                "raft leadership changed while verifying snapshot capabilities"
            ));
        }
        Ok(())
    }

    /// Verify that every voter has applied at least `index` while this node
    /// remains leader in the same term.
    ///
    /// Products use this barrier before a destructive transition whose query
    /// fence must be visible on every serving replica, not only on a quorum.
    pub async fn require_applied_index_on_all_voters(&self, index: Index) -> Result<()> {
        let _operation = self.shared.begin_coordinated_peer_work()?;
        let (term, voters) = {
            let node = self.shared.node.lock().await;
            if !node.is_leader() && node.conf_state().membership.voters.len() > 1 {
                return Err(anyhow!(
                    "only the Raft leader can verify voter applied indexes"
                ));
            }
            let local_applied = self.shared.completed_applied_index();
            if local_applied < index {
                return Err(anyhow!(
                    "raft: local voter {} has applied index {local_applied}, below required index {index}",
                    self.shared.id
                ));
            }
            (
                node.current_term(),
                node.conf_state().membership.voters.clone(),
            )
        };
        for voter in voters
            .iter()
            .copied()
            .filter(|voter| *voter != self.shared.id)
        {
            let status = self.shared.request_status(voter).await?;
            if status.applied_index < index {
                return Err(anyhow!(
                    "raft: voter {voter} has applied index {}, below required index {index}",
                    status.applied_index
                ));
            }
        }
        let node = self.shared.node.lock().await;
        if node.current_term() != term
            || (!node.is_leader() && node.conf_state().membership.voters.len() > 1)
        {
            return Err(anyhow!(
                "raft leadership changed while verifying voter applied indexes"
            ));
        }
        Ok(())
    }
}
