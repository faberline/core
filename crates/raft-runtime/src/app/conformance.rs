//! The public `DeterministicHost` constructors: they hand the deterministic
//! conformance host a `RaftStore` as its storage port.

use std::any::Any;
use std::sync::Arc;

use raft_core::{Membership, NodeId};

use crate::application::RaftStateMachine;
use crate::infrastructure::RaftStore;
use crate::interfaces::{DeterministicHost, StepError};

impl DeterministicHost {
    /// Open a host from an empty or durable store.  Unlike the historical
    /// production `spawn_inner`, this test host surfaces a corrupt-store load.
    pub fn open(
        id: NodeId,
        membership: Membership,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
    ) -> Result<Self, StepError> {
        Self::open_with_envelope_epoch(id, membership, store, sm, id as u32)
    }

    /// Open with a trace-owned epoch.  Assign a new epoch when a trace drops
    /// and reopens a host so envelope ids remain unique and replayable.
    pub fn open_with_envelope_epoch(
        id: NodeId,
        membership: Membership,
        store: RaftStore,
        sm: Arc<dyn RaftStateMachine>,
        envelope_epoch: u32,
    ) -> Result<Self, StepError> {
        Self::from_storage(id, membership, Box::new(store), sm, envelope_epoch)
    }

    /// The store this host was opened over.
    pub fn store(&self) -> &RaftStore {
        let storage: &dyn Any = self.storage();
        storage
            .downcast_ref::<RaftStore>()
            .expect("raft: every public DeterministicHost constructor stores in a RaftStore")
    }
}
