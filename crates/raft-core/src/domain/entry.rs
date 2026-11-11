use serde::{Deserialize, Serialize};

use super::ids::{Index, Term};

/// Discriminator distinguishing client commands, internal configuration entries and leader no-op entries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    #[default]
    Command,
    Config,
    Noop,
}

/// One replicated command entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaftEntry {
    pub term: Term,
    pub index: Index,
    pub command: Vec<u8>,
    #[serde(default)]
    pub kind: EntryKind,
}
