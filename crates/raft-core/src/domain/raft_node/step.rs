use super::RaftNode;
use crate::domain::ids::NodeId;
use crate::domain::message::{Outgoing, RaftMsg};
use crate::domain::role::Role;
use crate::domain::timing::HEARTBEAT_INTERVAL_TICKS;

impl RaftNode {
    /// Drain messages the driver must deliver.
    pub fn take_outgoing(&mut self) -> Vec<Outgoing> {
        std::mem::take(&mut self.outbox)
    }

    pub(super) fn send(&mut self, to: NodeId, msg: RaftMsg) {
        self.outbox.push(Outgoing { to, msg });
    }

    /// Advance one logical tick: leaders heartbeat, voters may start an election.
    pub fn tick(&mut self) {
        self.election_elapsed += 1;
        self.heartbeat_elapsed += 1;
        if self.role == Role::Leader {
            if self.transfer_in_flight.is_some() {
                self.transfer_elapsed += 1;
                if self.transfer_elapsed >= self.election_timeout {
                    self.transfer_in_flight = None;
                    self.transfer_elapsed = 0;
                }
            }
            if self.heartbeat_elapsed >= HEARTBEAT_INTERVAL_TICKS {
                self.heartbeat_elapsed = 0;
                self.broadcast_append();
            }
        } else if self.is_voter && self.election_elapsed >= self.election_timeout {
            self.start_election();
        }
    }

    /// Feed an incoming message from `from`.
    pub fn handle(&mut self, from: NodeId, msg: RaftMsg) {
        match msg {
            RaftMsg::Vote(req) => self.handle_vote(from, req),
            RaftMsg::VoteResp(resp) => self.handle_vote_resp(from, resp),
            RaftMsg::Append(req) => self.handle_append(req),
            RaftMsg::AppendResp(resp) => self.handle_append_resp(from, resp),
            RaftMsg::InstallSnapshot(req) => self.handle_install_snapshot(req),
            RaftMsg::InstallSnapshotResp(resp) => self.handle_install_snapshot_resp(from, resp),
            RaftMsg::TimeoutNow(req) => self.handle_timeout_now(req),
        }
    }
}
