use super::RaftNode;
use crate::domain::ids::{Index, NodeId};
use crate::domain::message::{InstallSnapshotReq, InstallSnapshotResp, RaftMsg};
use crate::domain::role::Role;

impl RaftNode {
    /// A snapshot received from a leader, for the consumer to load into its state
    /// machine (call once after [`handle`] processes an `InstallSnapshot`).
    pub fn take_installed_snapshot(&mut self) -> Option<Vec<u8>> {
        self.installed_snapshot.take()
    }

    /// Reject an incoming snapshot after the consumer could not restore its
    /// state-machine bytes.
    ///
    /// The higher term and leader identity still take effect, but the local
    /// snapshot index and resident log stay unchanged. The response advertises
    /// the old index so the leader retries instead of treating this voter as
    /// caught up.
    pub fn reject_install_snapshot(&mut self, req: InstallSnapshotReq) {
        if req.term < self.current_term {
            let (term, snapshot_index) = (self.current_term, self.snapshot_index);
            self.send(
                req.leader,
                RaftMsg::InstallSnapshotResp(InstallSnapshotResp {
                    term,
                    accepted: false,
                    snapshot_index,
                }),
            );
            return;
        }
        self.step_down(req.term);
        self.leader_id = Some(req.leader);
        let (term, snapshot_index) = (self.current_term, self.snapshot_index);
        self.send(
            req.leader,
            RaftMsg::InstallSnapshotResp(InstallSnapshotResp {
                term,
                accepted: false,
                snapshot_index,
            }),
        );
    }

    /// Compact the log up through `up_to` (must be applied): the consumer has
    /// snapshotted its state machine to `snapshot` bytes, so entries `<= up_to`
    /// can be dropped. The snapshot is what a leader later ships to a follower
    /// whose next index has been compacted away.
    pub fn compact(&mut self, up_to: Index, snapshot: Vec<u8>) {
        if up_to <= self.snapshot_index || up_to > self.last_applied {
            return;
        }
        let term = self.term_at(up_to);
        let drop = (up_to - self.snapshot_index) as usize;
        let drop = drop.min(self.log.len());
        let removed = self.log[..drop]
            .iter()
            .map(|entry| entry.command.len())
            .sum::<usize>();
        self.log.drain(0..drop);
        self.resident_log_bytes = self.resident_log_bytes.saturating_sub(removed);
        self.snapshot_index = up_to;
        self.snapshot_term = term;
        self.snapshot = snapshot;
    }

    pub(super) fn handle_install_snapshot(&mut self, req: InstallSnapshotReq) {
        if req.term < self.current_term {
            let (term, si) = (self.current_term, self.snapshot_index);
            self.send(
                req.leader,
                RaftMsg::InstallSnapshotResp(InstallSnapshotResp {
                    term,
                    accepted: false,
                    snapshot_index: si,
                }),
            );
            return;
        }
        self.step_down(req.term);
        self.leader_id = Some(req.leader);
        let accepted = if req.snapshot_index < self.snapshot_index {
            // A later state-machine snapshot supersedes this request.
            true
        } else if req.snapshot_index == self.snapshot_index {
            // Raft indexes are immutable identities. A retry is idempotent
            // only when its term and bytes are exactly the same.
            req.snapshot_term == self.snapshot_term && req.data == self.snapshot
        } else {
            // Keep a matching suffix when this follower was already at or
            // beyond the prospective snapshot point. This lets a leader first
            // prove that every voter accepted a checkpoint and only then drop
            // its own prefix, without deleting later entries on a caught-up
            // follower.
            let retained_suffix = if req.snapshot_index <= self.last_index()
                && self.term_at(req.snapshot_index) == req.snapshot_term
            {
                let first = (req.snapshot_index - self.snapshot_index) as usize;
                self.log[first.min(self.log.len())..].to_vec()
            } else {
                Vec::new()
            };
            self.log = retained_suffix;
            self.resident_log_bytes = self.log.iter().map(|entry| entry.command.len()).sum();
            self.snapshot_index = req.snapshot_index;
            self.snapshot_term = req.snapshot_term;
            self.snapshot = req.data.clone();
            self.installed_snapshot = Some(req.data);
            if self.commit_index < req.snapshot_index {
                self.commit_index = req.snapshot_index;
            }
            self.commit_index = self.commit_index.min(self.last_index());
            self.last_applied = req.snapshot_index;
            true
        };
        let (term, si) = (self.current_term, self.snapshot_index);
        self.send(
            req.leader,
            RaftMsg::InstallSnapshotResp(InstallSnapshotResp {
                term,
                accepted,
                snapshot_index: si,
            }),
        );
    }

    pub(super) fn handle_install_snapshot_resp(&mut self, from: NodeId, resp: InstallSnapshotResp) {
        if resp.term > self.current_term {
            self.step_down(resp.term);
            return;
        }
        if self.role != Role::Leader || resp.term != self.current_term {
            return;
        }
        if !resp.accepted {
            return;
        }
        let m = resp.snapshot_index;
        if m > *self.match_index.get(&from).unwrap_or(&0) {
            self.match_index.insert(from, m);
        }
        self.next_index.insert(from, m + 1);
        let old = self.commit_index;
        self.maybe_commit();
        if self.commit_index > old {
            self.broadcast_append();
        } else if *self.next_index.get(&from).unwrap_or(&1) <= self.last_index() {
            self.send_append_to(from);
        }
    }
}
