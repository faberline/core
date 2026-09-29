use serde::{Deserialize, Serialize};

use super::conf_state::ConfState;
use super::entry::RaftEntry;
use super::ids::{Index, NodeId, Term};

/// The durable hard state of a Raft node: what must survive a restart so the
/// node never double-votes in a term or forgets acknowledged entries. Carries
/// the compaction point + snapshot bytes so a restarted node can still serve
/// lagging followers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedState {
    pub term: Term,
    pub voted_for: Option<NodeId>,
    pub log: Vec<RaftEntry>,
    /// Highest index known committed when the hard state was saved. Persisting
    /// this lets a cold host replay committed resident entries before serving
    /// reads, rather than waiting for a new-term proposal to re-establish the
    /// commit watermark.
    #[serde(default)]
    pub commit_index: Index,
    #[serde(default)]
    pub snapshot_index: Index,
    #[serde(default)]
    pub snapshot_term: Term,
    #[serde(default)]
    pub snapshot: Vec<u8>,
    #[serde(default)]
    pub conf: Option<ConfState>,
}

/// Borrowed durable state used by runtimes that must persist a large log
/// without cloning it on every heartbeat or proposal.
pub struct PersistedStateRef<'a> {
    pub term: Term,
    pub voted_for: Option<NodeId>,
    pub log: &'a [RaftEntry],
    pub commit_index: Index,
    pub snapshot_index: Index,
    pub snapshot_term: Term,
    pub snapshot: &'a [u8],
    pub conf: Option<&'a ConfState>,
}
