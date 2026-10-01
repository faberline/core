use super::RaftNode;
use crate::domain::conf_state::ConfState;
use crate::domain::entry::EntryKind;
use crate::domain::ids::{Index, NodeId};
use crate::domain::membership::Membership;
use crate::domain::refusal::{DemotionRefused, PromotionRefused, RemovalRefused};
use crate::domain::role::Role;

impl RaftNode {
    /// Promote a caught-up learner to voter through a joint configuration.
    pub fn promote_learner(&mut self, peer: NodeId) -> Result<Index, PromotionRefused> {
        if self.role != Role::Leader {
            return Err(PromotionRefused::NotLeader);
        }
        if self.is_joint() || self.transfer_in_flight.is_some() {
            return Err(PromotionRefused::TransitionInFlight);
        }
        if self
            .log
            .iter()
            .any(|e| e.index > self.commit_index && e.kind == EntryKind::Config)
        {
            return Err(PromotionRefused::TransitionInFlight);
        }
        let matched = self.learner_matched(peer).unwrap_or(Index::new(0));
        let target = self.learner_read_target(peer).unwrap_or(Index::new(0));
        if matched < target {
            return Err(PromotionRefused::NotCaughtUp { matched, target });
        }
        let mut new_voters = self.conf_state.membership.voters().to_vec();
        if !new_voters.contains(&peer) {
            new_voters.push(peer);
            new_voters.sort_unstable();
        }
        let mut new_learners = self.conf_state.membership.learners().to_vec();
        new_learners.retain(|l| *l != peer);

        let outgoing = Some(self.conf_state.membership.voters().to_vec());
        let conf = ConfState {
            membership: Membership::new(new_voters, new_learners),
            outgoing,
            generation: self.conf_state.generation + 1,
        };
        let idx = self
            .propose_config(conf)
            // unreachable: propose_config returns None only when self.role != Role::Leader or self.transfer_in_flight.is_some(), which are excluded above by self.role != Role::Leader and self.is_joint() || self.transfer_in_flight.is_some().
            .ok_or(PromotionRefused::TransitionInFlight)?;
        Ok(idx)
    }

    /// Demote a voter to a learner through a joint configuration (#3572).
    pub fn demote_voter(&mut self, peer: NodeId) -> Result<Index, DemotionRefused> {
        if self.role != Role::Leader {
            return Err(DemotionRefused::NotLeader);
        }
        if peer == self.id {
            return Err(DemotionRefused::IsTheLeader { target: peer });
        }
        if self.is_joint() || self.transfer_in_flight.is_some() {
            return Err(DemotionRefused::TransitionInFlight);
        }
        if self
            .log
            .iter()
            .any(|e| e.index > self.commit_index && e.kind == EntryKind::Config)
        {
            return Err(DemotionRefused::TransitionInFlight);
        }
        if !self.conf_state.membership.voters().contains(&peer) {
            return Err(DemotionRefused::NotAVoter { target: peer });
        }
        let mut new_voters = self.conf_state.membership.voters().to_vec();
        new_voters.retain(|v| *v != peer);
        if new_voters.is_empty() {
            return Err(DemotionRefused::WouldEmptyVoterSet { target: peer });
        }
        let n = self.conf_state.membership.voters().len();
        let before = n.saturating_sub(n / 2 + 1);
        let after = (n - 1).saturating_sub((n - 1) / 2 + 1);
        if after < before {
            return Err(DemotionRefused::ToleranceWouldDrop { before, after });
        }
        let mut new_learners = self.conf_state.membership.learners().to_vec();
        if !new_learners.contains(&peer) {
            new_learners.push(peer);
            new_learners.sort_unstable();
        }

        let outgoing = Some(self.conf_state.membership.voters().to_vec());
        let conf = ConfState {
            membership: Membership::new(new_voters, new_learners),
            outgoing,
            generation: self.conf_state.generation + 1,
        };
        let idx = self
            .propose_config(conf)
            // unreachable: propose_config returns None only when self.role != Role::Leader or self.transfer_in_flight.is_some(), which are excluded above by self.role != Role::Leader and self.is_joint() || self.transfer_in_flight.is_some().
            .ok_or(DemotionRefused::TransitionInFlight)?;
        Ok(idx)
    }

    /// Remove a member from the group through a joint configuration (#3572).
    pub fn remove_member(&mut self, peer: NodeId) -> Result<Index, RemovalRefused> {
        if self.role != Role::Leader {
            return Err(RemovalRefused::NotLeader);
        }
        if peer == self.id {
            return Err(RemovalRefused::IsTheLeader { target: peer });
        }
        if self.is_joint() || self.transfer_in_flight.is_some() {
            return Err(RemovalRefused::TransitionInFlight);
        }
        if self
            .log
            .iter()
            .any(|e| e.index > self.commit_index && e.kind == EntryKind::Config)
        {
            return Err(RemovalRefused::TransitionInFlight);
        }
        let is_voter = self.conf_state.membership.voters().contains(&peer);
        let is_learner = self.conf_state.membership.learners().contains(&peer);
        if !is_voter && !is_learner {
            return Err(RemovalRefused::NotAMember { target: peer });
        }
        let mut new_voters = self.conf_state.membership.voters().to_vec();
        new_voters.retain(|v| *v != peer);
        if is_voter {
            if new_voters.is_empty() {
                return Err(RemovalRefused::WouldEmptyVoterSet { target: peer });
            }
            let n = self.conf_state.membership.voters().len();
            let before = n.saturating_sub(n / 2 + 1);
            let after = (n - 1).saturating_sub((n - 1) / 2 + 1);
            if after < before {
                return Err(RemovalRefused::ToleranceWouldDrop { before, after });
            }
        }
        let mut new_learners = self.conf_state.membership.learners().to_vec();
        new_learners.retain(|l| *l != peer);

        let outgoing = Some(self.conf_state.membership.voters().to_vec());
        let conf = ConfState {
            membership: Membership::new(new_voters, new_learners),
            outgoing,
            generation: self.conf_state.generation + 1,
        };
        let idx = self
            .propose_config(conf)
            // unreachable: propose_config returns None only when self.role != Role::Leader or self.transfer_in_flight.is_some(), which are excluded above by self.role != Role::Leader and self.is_joint() || self.transfer_in_flight.is_some().
            .ok_or(RemovalRefused::TransitionInFlight)?;
        Ok(idx)
    }
}
