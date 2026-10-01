use serde::{Deserialize, Serialize};

use super::ids::{Index, Term};

/// Discriminator distinguishing client commands from internal configuration entries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    #[default]
    Command,
    Config,
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
