//! `RaftStore` as the raft-core storage port a host persists through.

use raft_core::{CommandLease, PinnedCommand, RaftStorage};

use super::*;

impl RaftStorage for RaftStore {
    fn load(&self) -> io::Result<Option<PersistedState>> {
        RaftStore::load(self)
    }

    fn save(&self, state: &PersistedStateRef<'_>) -> io::Result<()> {
        self.save_ref(state)
    }

    fn pin_committed_command(
        &self,
        index: Index,
        term: Term,
    ) -> io::Result<Box<dyn PinnedCommand>> {
        Ok(Box::new(RaftStore::pin_committed_command(
            self, index, term,
        )?))
    }
}

impl PinnedCommand for CommittedCommandPin {
    fn map(self: Box<Self>) -> io::Result<Box<dyn CommandLease>> {
        Ok(Box::new(CommittedCommandPin::map(*self)?))
    }
}

impl CommandLease for CommittedCommandLease {
    fn command(&self) -> &[u8] {
        CommittedCommandLease::command(self)
    }
}
