use std::collections::HashMap;
use std::sync::Arc;

use raft_core::{Index, Membership, NodeId, RaftNode, Role};

use super::{
    envelope_kind, fingerprint, ConformanceMembershipError, ConformanceRole, EnvelopeMeta,
    NodeView, PendingEnvelope, StateMachineOperation, StepError,
};
use crate::application::{
    apply_ready, cold_start, persist_node, HostStorage, PeerLaneQueue, RaftStateMachine,
    SnapshotPolicy, SNAPSHOT_CHUNK_SIZE,
};

/// One deterministic Raft host.  Drop it to model a crash, then call `open`
/// with the same [`RaftStore`](crate::RaftStore) and a new state machine to
/// model restart.
pub struct DeterministicHost {
    id: NodeId,
    node: RaftNode,
    storage: Box<dyn HostStorage>,
    sm: Arc<dyn RaftStateMachine>,
    lanes: HashMap<NodeId, PeerLaneQueue>,
    envelope_epoch: u32,
    next_envelope_id: u32,
}

impl DeterministicHost {
    /// Open a host over `storage`.  The public `open` constructors in the
    /// composition root (`src/app/conformance.rs`) hand it a `RaftStore`.
    pub(crate) fn from_storage(
        id: NodeId,
        membership: Membership,
        storage: Box<dyn HostStorage>,
        sm: Arc<dyn RaftStateMachine>,
        envelope_epoch: u32,
    ) -> Result<Self, StepError> {
        let mut node = match storage.load().map_err(|e| StepError::Store {
            operation: "load",
            kind: e.kind(),
        })? {
            Some(state) => RaftNode::from_persisted(id, &membership, state),
            None => RaftNode::new(id, &membership),
        };
        cold_start(&mut node, sm.as_ref(), true).map_err(|e| StepError::StateMachine {
            operation: "cold-start",
            message: e.to_string(),
        })?;
        let mut host = Self {
            id,
            node,
            storage,
            sm,
            lanes: HashMap::new(),
            envelope_epoch,
            next_envelope_id: 1,
        };
        host.pump()?;
        Ok(host)
    }

    /// Advance exactly one logical Raft tick.
    pub fn tick(&mut self) -> Result<(), StepError> {
        self.node.tick();
        self.settle()
    }

    /// Move the node outbox into per-peer FIFO lanes.  Only consecutive,
    /// unsent Append messages coalesce because this uses `PeerLaneQueue`.
    pub fn pump(&mut self) -> Result<(), StepError> {
        for outgoing in self.node.take_outgoing() {
            self.lanes
                .entry(outgoing.to)
                .or_default()
                .enqueue(outgoing.msg);
        }
        self.persist()
    }

    /// Peer ids that have at least one deliverable envelope, in stable order.
    pub fn ready_peers(&self) -> Vec<NodeId> {
        let mut peers: Vec<_> = self
            .lanes
            .iter()
            .filter_map(|(&peer, lane)| (!lane.is_empty()).then_some(peer))
            .collect();
        peers.sort_unstable();
        peers
    }

    /// Exact number of envelopes still queued in the shared peer lanes.
    pub fn pending_len(&self) -> usize {
        self.lanes.values().map(PeerLaneQueue::len).sum()
    }

    /// Remove one pending envelope for `peer` from that peer's FIFO lane.
    pub fn take_next(&mut self, peer: NodeId) -> Option<PendingEnvelope> {
        let message = self.lanes.get_mut(&peer)?.dequeue()?;
        let meta = EnvelopeMeta {
            id: ((self.envelope_epoch as u64) << 32) | self.next_envelope_id as u64,
            from: self.id,
            to: peer,
            kind: envelope_kind(&message),
            fingerprint: fingerprint(&message),
        };
        self.next_envelope_id = self
            .next_envelope_id
            .checked_add(1)
            .expect("trace envelope id space exhausted");
        Some(PendingEnvelope { meta, message })
    }

    /// Deliver one opaque envelope to this host.
    pub fn receive(&mut self, envelope: PendingEnvelope) -> Result<(), StepError> {
        if envelope.meta.to != self.id {
            return Err(StepError::WrongRecipient {
                expected: self.id,
                actual: envelope.meta.to,
            });
        }
        self.node.handle(envelope.meta.from, envelope.message);
        self.settle()
    }

    pub fn try_propose(&mut self, operation: StateMachineOperation) -> Result<Index, StepError> {
        let StateMachineOperation::Command(command) = operation;
        let index = self.node.propose(command).ok_or(StepError::NotLeader)?;
        self.settle()?;
        Ok(index)
    }

    pub fn try_add_learner(&mut self, peer: NodeId) -> Result<Index, StepError> {
        if !self.node.is_leader() {
            return Err(StepError::NotLeader);
        }
        let index = self.node.add_learner(peer).ok_or(StepError::Membership(
            ConformanceMembershipError::AddLearnerRefused,
        ))?;
        self.settle()?;
        Ok(index)
    }

    pub fn promote_learner(&mut self, peer: NodeId) -> Result<Index, StepError> {
        if !self.node.is_leader() {
            return Err(StepError::NotLeader);
        }
        let index = self
            .node
            .promote_learner(peer)
            .map_err(|e| StepError::Membership(ConformanceMembershipError::Promote(e)))?;
        self.settle()?;
        Ok(index)
    }

    pub fn demote_voter(&mut self, peer: NodeId) -> Result<Index, StepError> {
        if !self.node.is_leader() {
            return Err(StepError::NotLeader);
        }
        let index = self
            .node
            .demote_voter(peer)
            .map_err(|e| StepError::Membership(ConformanceMembershipError::Demote(e)))?;
        self.settle()?;
        Ok(index)
    }

    pub fn remove_member(&mut self, peer: NodeId) -> Result<Index, StepError> {
        if !self.node.is_leader() {
            return Err(StepError::NotLeader);
        }
        let index = self
            .node
            .remove_member(peer)
            .map_err(|e| StepError::Membership(ConformanceMembershipError::Remove(e)))?;
        self.settle()?;
        Ok(index)
    }

    /// Capture the state-machine snapshot and compact all applied entries.
    pub fn snapshot_and_compact(&mut self) -> Result<Index, StepError> {
        let applied = self.sm.applied_index();
        if applied <= self.node.snapshot_index() {
            return Ok(self.node.snapshot_index());
        }
        let mut sink = crate::ChunkSink::new(SNAPSHOT_CHUNK_SIZE);
        self.sm
            .snapshot(&mut sink)
            .map_err(|e| StepError::StateMachine {
                operation: "snapshot",
                message: e.to_string(),
            })?;
        self.node.compact(applied, sink.into_bytes());
        self.settle()?;
        Ok(applied)
    }

    pub fn view(&self) -> NodeView {
        NodeView {
            id: self.id,
            role: match self.node.role() {
                Role::Follower => ConformanceRole::Follower,
                Role::Candidate => ConformanceRole::Candidate,
                Role::Leader => ConformanceRole::Leader,
            },
            term: self.node.current_term(),
            leader: self.node.leader(),
            commit_index: self.node.commit_index(),
            last_index: self.node.last_index(),
            snapshot_index: self.node.snapshot_index(),
            resident_log_entries: self.node.log_len(),
            membership: self.node.conf_state().membership.clone(),
            joint: self.node.is_joint(),
        }
    }

    /// The storage port this host was opened over.
    pub(crate) fn storage(&self) -> &dyn HostStorage {
        self.storage.as_ref()
    }

    fn settle(&mut self) -> Result<(), StepError> {
        self.persist()?;
        apply_ready(
            &mut self.node,
            self.sm.as_ref(),
            None,
            SnapshotPolicy::Disabled,
            true,
        )
        .map_err(|e| StepError::StateMachine {
            operation: "apply-ready",
            message: e.to_string(),
        })?;
        // Applying a configuration entry can append the leave-joint entry.
        // Persist it before exposing a new envelope or returning to the trace.
        self.pump()
    }

    fn persist(&self) -> Result<(), StepError> {
        persist_node(self.storage.as_ref(), &self.node).map_err(|e| StepError::Store {
            operation: "save",
            kind: e.kind(),
        })
    }
}
