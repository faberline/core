//! The step-driven Raft node: its state, and one `impl RaftNode` block per
//! responsibility in the child modules.

mod accessors;
mod apply;
mod commit_advance;
mod conf_adoption;
mod election;
mod leadership_transfer;
mod learner_readiness;
mod membership_change;
mod persistence;
mod proposal;
mod replication;
mod snapshot;
mod step;

use std::collections::{HashMap, HashSet};

use super::conf_state::ConfState;
use super::entry::RaftEntry;
use super::ids::{Index, NodeId, Term};
use super::membership::Membership;
use super::message::Outgoing;
use super::role::Role;
use super::timing::ELECTION_TIMEOUT_FLOOR_TICKS;

/// A single Raft-group participant.
pub struct RaftNode {
    id: NodeId,
    peers: Vec<NodeId>, // all other members (voters + learners)
    is_voter: bool,
    conf_state: ConfState,

    role: Role,
    current_term: Term,
    voted_for: Option<NodeId>,
    /// In-memory log; `log[0]` has index `snapshot_index + 1`.
    log: Vec<RaftEntry>,
    resident_log_bytes: usize,
    commit_index: Index,
    last_applied: Index,

    // compaction
    snapshot_index: Index,
    snapshot_term: Term,
    snapshot: Vec<u8>,
    installed_snapshot: Option<Vec<u8>>,

    // leader-only, per peer
    next_index: HashMap<NodeId, Index>,
    match_index: HashMap<NodeId, Index>,
    learner_read_targets: HashMap<NodeId, Index>,

    // election
    votes: HashSet<NodeId>,
    election_elapsed: u64,
    election_timeout: u64,
    heartbeat_elapsed: u64,
    /// Last known leader for this term (drives producer redirect-to-leader).
    leader_id: Option<NodeId>,

    // leadership transfer
    transfer_in_flight: Option<NodeId>,
    transfer_elapsed: u64,

    outbox: Vec<Outgoing>,
}

impl RaftNode {
    /// Create a node `id` within `membership` (starts as Follower at term 0).
    pub fn new(id: NodeId, membership: &Membership) -> RaftNode {
        let mut members: Vec<NodeId> = membership
            .voters()
            .iter()
            .chain(membership.learners().iter())
            .copied()
            .collect();
        members.sort_unstable();
        let peers = members.into_iter().filter(|m| *m != id).collect();
        let mut learner_read_targets = HashMap::new();
        for &l in membership.learners() {
            learner_read_targets.insert(l, 0);
        }
        RaftNode {
            id,
            peers,
            is_voter: membership.voters().contains(&id),
            conf_state: ConfState {
                membership: membership.clone(),
                outgoing: None,
                generation: 0,
            },
            role: Role::Follower,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            resident_log_bytes: 0,
            commit_index: 0,
            last_applied: 0,
            snapshot_index: 0,
            snapshot_term: 0,
            snapshot: Vec::new(),
            installed_snapshot: None,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            learner_read_targets,
            votes: HashSet::new(),
            election_elapsed: 0,
            // distinct per node so one voter always times out first.
            election_timeout: ELECTION_TIMEOUT_FLOOR_TICKS + id.get(),
            heartbeat_elapsed: 0,
            leader_id: None,
            transfer_in_flight: None,
            transfer_elapsed: 0,
            outbox: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
