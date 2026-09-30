use super::RaftNode;
use crate::domain::ids::{Index, NodeId, Term};
use crate::domain::message::{RaftMsg, VoteReq, VoteResp};
use crate::domain::role::Role;

impl RaftNode {
    pub(super) fn start_election(&mut self) {
        self.current_term = self.current_term.next();
        self.role = Role::Candidate;
        self.voted_for = Some(self.id);
        self.leader_id = None;
        self.votes.clear();
        self.votes.insert(self.id);
        self.election_elapsed = 0;
        let (lli, llt) = (self.last_index(), self.last_term());
        let term = self.current_term;
        let mut vote_targets = self.conf_state.membership.voters().to_vec();
        if let Some(outgoing) = &self.conf_state.outgoing {
            vote_targets.extend(outgoing);
        }
        vote_targets.sort_unstable();
        vote_targets.dedup();
        let peers: Vec<NodeId> = vote_targets.into_iter().filter(|v| *v != self.id).collect();
        for v in peers {
            self.send(
                v,
                RaftMsg::Vote(VoteReq {
                    term,
                    candidate: self.id,
                    last_log_index: lli,
                    last_log_term: llt,
                }),
            );
        }
        // A sole voter wins immediately.
        self.maybe_become_leader();
    }

    fn maybe_become_leader(&mut self) {
        if self.role != Role::Candidate {
            return;
        }
        let incoming_granted = self
            .votes
            .iter()
            .filter(|v| self.conf_state.membership.voters().contains(v))
            .count();
        let incoming_maj = self.conf_state.membership.voters().len() / 2 + 1;
        let outgoing_satisfied = match &self.conf_state.outgoing {
            Some(outgoing) => {
                let outgoing_granted = self.votes.iter().filter(|v| outgoing.contains(v)).count();
                let outgoing_maj = outgoing.len() / 2 + 1;
                outgoing_granted >= outgoing_maj
            }
            None => true,
        };
        if incoming_granted >= incoming_maj && outgoing_satisfied {
            self.become_leader();
        }
    }

    pub(super) fn become_leader(&mut self) {
        self.role = Role::Leader;
        self.leader_id = Some(self.id);
        let next = self.last_index().next();
        self.next_index.clear();
        self.match_index.clear();
        for p in self.peers.clone() {
            self.next_index.insert(p, next);
            self.match_index.insert(p, Index::new(0));
        }
        self.heartbeat_elapsed = 0;
        self.transfer_in_flight = None;
        self.transfer_elapsed = 0;
        self.broadcast_append();
        if self.is_joint() {
            self.check_leave_joint();
        }
    }

    pub(super) fn step_down(&mut self, term: Term) {
        if term > self.current_term {
            self.current_term = term;
            self.voted_for = None;
        }
        self.role = Role::Follower;
        self.election_elapsed = 0;
        self.transfer_in_flight = None;
        self.transfer_elapsed = 0;
    }

    pub(super) fn handle_vote(&mut self, from: NodeId, req: VoteReq) {
        if req.term > self.current_term {
            self.step_down(req.term);
        }
        let up_to_date = req.last_log_term > self.last_term()
            || (req.last_log_term == self.last_term() && req.last_log_index >= self.last_index());
        let grant = req.term == self.current_term
            && (self.voted_for.is_none() || self.voted_for == Some(req.candidate))
            && up_to_date;
        if grant {
            self.voted_for = Some(req.candidate);
            self.election_elapsed = 0;
        }
        let term = self.current_term;
        self.send(
            from,
            RaftMsg::VoteResp(VoteResp {
                term,
                granted: grant,
            }),
        );
    }

    pub(super) fn handle_vote_resp(&mut self, from: NodeId, resp: VoteResp) {
        if resp.term > self.current_term {
            self.step_down(resp.term);
            return;
        }
        if self.role == Role::Candidate && resp.term == self.current_term && resp.granted {
            self.votes.insert(from);
            self.maybe_become_leader();
        }
    }
}
