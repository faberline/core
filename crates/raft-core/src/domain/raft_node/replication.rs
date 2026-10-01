use super::RaftNode;
use crate::domain::entry::RaftEntry;
use crate::domain::ids::{Index, NodeId};
use crate::domain::message::{AppendReq, AppendResp, InstallSnapshotReq, RaftMsg};
use crate::domain::role::Role;

impl RaftNode {
    pub(super) fn broadcast_append(&mut self) {
        for p in self.peers.clone() {
            self.send_append_to(p);
        }
    }

    pub(super) fn send_append_to(&mut self, peer: NodeId) {
        let next = *self
            .next_index
            .get(&peer)
            .unwrap_or(&self.last_index().next());
        // Needed entries compacted away → ship the snapshot instead.
        if next <= self.snapshot_index {
            let (term, si, st) = (self.current_term, self.snapshot_index, self.snapshot_term);
            let data = self.snapshot.clone();
            self.send(
                peer,
                RaftMsg::InstallSnapshot(InstallSnapshotReq {
                    term,
                    leader: self.id,
                    snapshot_index: si,
                    snapshot_term: st,
                    data,
                }),
            );
            return;
        }
        let prev_index = next.saturating_sub(1);
        let prev_term = self.term_at(prev_index);
        let entries: Vec<RaftEntry> = self
            .log
            .iter()
            .filter(|e| e.index >= next)
            .cloned()
            .collect();
        let (term, commit) = (self.current_term, self.commit_index);
        self.send(
            peer,
            RaftMsg::Append(AppendReq {
                term,
                leader: self.id,
                prev_log_index: prev_index,
                prev_log_term: prev_term,
                entries,
                leader_commit: commit,
            }),
        );
    }

    pub(super) fn handle_append(&mut self, req: AppendReq) {
        let leader = req.leader;
        if req.term < self.current_term {
            let term = self.current_term;
            self.send(
                leader,
                RaftMsg::AppendResp(AppendResp {
                    term,
                    success: false,
                    match_index: Index::new(0),
                }),
            );
            return;
        }
        // Valid leader for this (or a newer) term: become its follower.
        self.step_down(req.term);
        self.leader_id = Some(leader);

        // Log matching: the entry preceding the new ones must agree. Anything at
        // or below our snapshot point is implicitly matched.
        if req.prev_log_index > self.last_index()
            || (req.prev_log_index > self.snapshot_index
                && self.term_at(req.prev_log_index) != req.prev_log_term)
        {
            let term = self.current_term;
            // Report the greatest prefix this request can safely assume did
            // not match. The leader uses this both as a fast backoff hint and
            // to discard a failure response that arrived after a newer
            // successful AppendEntries response for the same peer.
            let match_index = req.prev_log_index.saturating_sub(1).min(self.last_index());
            self.send(
                leader,
                RaftMsg::AppendResp(AppendResp {
                    term,
                    success: false,
                    match_index,
                }),
            );
            return;
        }

        // Append, skipping entries already covered by the snapshot and truncating
        // any conflicting suffix.
        for e in &req.entries {
            if e.index <= self.snapshot_index {
                continue;
            }
            let pos = (e.index.get() - self.snapshot_index.get() - 1) as usize;
            if pos < self.log.len() {
                if self.log[pos].term != e.term {
                    let removed = self.log[pos..]
                        .iter()
                        .map(|entry| entry.command.len())
                        .sum::<usize>();
                    self.log.truncate(pos);
                    self.resident_log_bytes = self.resident_log_bytes.saturating_sub(removed);
                    self.resident_log_bytes =
                        self.resident_log_bytes.saturating_add(e.command.len());
                    self.log.push(e.clone());
                }
            } else {
                self.resident_log_bytes = self.resident_log_bytes.saturating_add(e.command.len());
                self.log.push(e.clone());
            }
        }
        let match_index = Index::new(req.prev_log_index.get() + req.entries.len() as u64);
        if req.leader_commit > self.commit_index {
            self.commit_index = req.leader_commit.min(self.last_index());
        }
        let term = self.current_term;
        self.send(
            leader,
            RaftMsg::AppendResp(AppendResp {
                term,
                success: true,
                match_index,
            }),
        );
    }

    pub(super) fn handle_append_resp(&mut self, from: NodeId, resp: AppendResp) {
        if resp.term > self.current_term {
            self.step_down(resp.term);
            return;
        }
        if self.role != Role::Leader || resp.term != self.current_term {
            return;
        }
        if resp.success {
            // Multiple h2 requests to one peer can complete out of order.
            // Replication progress is monotonic: a stale success must never
            // move match_index/next_index behind a newer acknowledgement.
            let matched = self.match_index.entry(from).or_insert(Index::new(0));
            *matched = (*matched).max(resp.match_index);
            let next = self.next_index.entry(from).or_insert(Index::new(1));
            *next = (*next).max(matched.saturating_add(1));
            let old = self.commit_index;
            self.maybe_commit();
            if self.commit_index > old {
                // Propagate the new commit to everyone.
                self.broadcast_append();
            } else if *self.next_index.get(&from).unwrap_or(&Index::new(1)) <= self.last_index() {
                self.send_append_to(from);
            }
        } else {
            // Log mismatch: back off and retry (snapshot kicks in once next falls
            // to or below the compaction point). Ignore a delayed failure for
            // a prefix a newer response already proved replicated.
            if resp.match_index < *self.match_index.get(&from).unwrap_or(&Index::new(0)) {
                return;
            }
            let n = self.next_index.entry(from).or_insert(Index::new(1));
            *n = (*n)
                .saturating_sub(1)
                .min(resp.match_index.saturating_add(1))
                .max(Index::new(1));
            self.send_append_to(from);
        }
    }
}
