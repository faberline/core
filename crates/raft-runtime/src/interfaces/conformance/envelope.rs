use raft_core::NodeId;

/// Stable on-disk trace schema for adversarial-recovery replay files.
pub const TRACE_SCHEMA: &str = "raft-runtime/adversarial-recovery/v1";

/// The kind of one message scheduled by a deterministic host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnvelopeKind {
    Vote,
    VoteResponse,
    Append,
    AppendResponse,
    InstallSnapshot,
    InstallSnapshotResponse,
    TimeoutNow,
}

/// Stable metadata for a pending envelope.  The actual Raft wire message stays
/// opaque so a test can schedule it but cannot accidentally forge a different
/// message under the same identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvelopeMeta {
    pub id: u64,
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EnvelopeKind,
    pub fingerprint: String,
}
