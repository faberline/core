use super::RaftNode;
use crate::domain::ids::NodeId;
use crate::domain::message::{RaftMsg, TimeoutNowReq};
use crate::domain::refusal::TransferRefused;
use crate::domain::role::Role;

impl RaftNode {
    /// Return an eligible caught-up voter to receive leadership, or `None` if
    /// this node is not the leader, is the only voter, or no other voter has
    /// replicated this node's whole log (#3664).
    pub fn handoff_candidate(&self) -> Option<NodeId> {
        if self.role != Role::Leader {
            return None;
        }
        let last_index = self.last_index();
        self.conf_state
            .membership
            .voters
            .iter()
            .copied()
            .filter(|&id| id != self.id)
            .find(|&id| {
                let matched = self.match_index.get(&id).copied().unwrap_or(0);
                matched >= last_index
            })
    }

    /// Transfer leadership to a named caught-up voter (#3571).
    pub fn transfer_leadership(&mut self, target: NodeId) -> Result<(), TransferRefused> {
        if self.role != Role::Leader {
            return Err(TransferRefused::NotLeader);
        }
        if !self.conf_state.membership.voters.contains(&target) {
            return Err(TransferRefused::NotAVoter { target });
        }
        let matched = if target == self.id {
            self.last_index()
        } else {
            self.match_index.get(&target).copied().unwrap_or(0)
        };
        let last_index = self.last_index();
        if matched < last_index {
            return Err(TransferRefused::NotCaughtUp {
                target,
                matched,
                last_index,
            });
        }
        self.transfer_in_flight = Some(target);
        self.transfer_elapsed = 0;
        self.send(
            target,
            RaftMsg::TimeoutNow(TimeoutNowReq {
                term: self.current_term,
                leader: self.id,
            }),
        );
        Ok(())
    }

    pub(super) fn handle_timeout_now(&mut self, req: TimeoutNowReq) {
        if !self.is_voter {
            return;
        }
        if req.term < self.current_term {
            return;
        }
        if req.term > self.current_term {
            self.current_term = req.term;
            self.voted_for = None;
        }
        self.start_election();
    }
}
