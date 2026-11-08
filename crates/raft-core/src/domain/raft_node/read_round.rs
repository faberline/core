use super::RaftNode;
use crate::domain::ids::Index;
use crate::domain::refusal::ReadRefused;
use crate::domain::role::Role;

impl RaftNode {
    /// Leader: start a new read round and heartbeat every peer with it.
    /// Returns the round number. Refuses when not leader, or when the leader
    /// has not yet committed an entry of its own term (Raft section 6.4).
    pub fn start_read_round(&mut self) -> Result<u64, ReadRefused> {
        if self.role != Role::Leader {
            return Err(ReadRefused::NotLeader);
        }
        if self.commit_index == Index::new(0)
            || self.term_at(self.commit_index) != self.current_term
        {
            return Err(ReadRefused::NoCommitInTerm);
        }
        self.read_round = self.read_round.saturating_add(1);
        self.read_acks.clear();
        if self.is_voter {
            self.read_acks.insert(self.id);
        }
        self.note_read_quorum();
        self.broadcast_append();
        Ok(self.read_round)
    }

    /// True when a quorum has acked `round` (or a later round) in the current
    /// leadership. False for round 0, a round never started, or when this
    /// node is no longer leader.
    pub fn read_round_acked(&self, round: u64) -> bool {
        self.role == Role::Leader && round != 0 && round <= self.read_acked_round
    }

    /// Record the current round as acked once a quorum of voters has acked it.
    pub(super) fn note_read_quorum(&mut self) {
        if self.read_quorum_reached() {
            self.read_acked_round = self.read_round;
        }
    }

    fn read_quorum_reached(&self) -> bool {
        let voters = self.conf_state.membership.voters();
        let incoming = voters.iter().filter(|v| self.read_acks.contains(v)).count();
        if incoming < voters.len() / 2 + 1 {
            return false;
        }
        match &self.conf_state.outgoing {
            Some(out) => {
                let outgoing = out.iter().filter(|v| self.read_acks.contains(v)).count();
                let outgoing_maj = out.len() / 2 + 1;
                outgoing >= outgoing_maj
            }
            None => true,
        }
    }
}
