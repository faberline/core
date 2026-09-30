//! The durable-storage port a driver persists a [`RaftNode`](super::RaftNode)
//! through: its hard state, log and snapshot, and a pinned read of one
//! committed command.

use std::io;

use super::ids::{Index, Term};
use super::persisted_state::{PersistedState, PersistedStateRef};

/// Durable storage for one node's [`PersistedState`].
///
/// A driver saves [`RaftNode::persisted_ref`](super::RaftNode::persisted_ref)
/// before it sends the messages or applies the entries that state implies, and
/// loads it on restart with
/// [`RaftNode::from_persisted`](super::RaftNode::from_persisted).
pub trait RaftStorage: Send + Sync {
    /// The last saved state, or `None` for a node that never saved one.
    fn load(&self) -> io::Result<Option<PersistedState>>;

    /// Durably replace the saved state with `state`.
    fn save(&self, state: &PersistedStateRef<'_>) -> io::Result<()>;

    /// Pin the saved command of the committed entry at `index` in `term`, so
    /// a driver can apply it without holding the node's in-memory log.
    fn pin_committed_command(&self, index: Index, term: Term)
        -> io::Result<Box<dyn PinnedCommand>>;
}

/// A committed command that storage keeps readable until it is mapped.
pub trait PinnedCommand: Send {
    /// Read the pinned command.
    fn map(self: Box<Self>) -> io::Result<Box<dyn CommandLease>>;
}

/// The bytes of one committed command, held for as long as the lease lives.
pub trait CommandLease: Send {
    /// The command bytes exactly as they were proposed.
    fn command(&self) -> &[u8];
}
