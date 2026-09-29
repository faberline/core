//! Deterministic, in-process host for adversarial Raft recovery tests.
//!
//! This module deliberately has no runtime, socket, timer, or wall-clock
//! dependency.  A harness drives logical time with [`DeterministicHost::tick`]
//! and moves opaque envelopes itself.  It uses the production host's cold-start,
//! apply, persistence, and peer-lane primitives so its result is evidence about
//! the shipped driver rather than a second implementation of it.

use raft_core::RaftMsg;
use sha2::{Digest, Sha256};

mod deterministic_host;
mod envelope;
mod error;
mod operation;
mod view;

pub use deterministic_host::DeterministicHost;
pub use envelope::{EnvelopeKind, EnvelopeMeta, TRACE_SCHEMA};
pub use error::{ConformanceMembershipError, StepError};
pub use operation::StateMachineOperation;
pub use view::{ConformanceRole, NodeView};

/// An opaque, cloneable outbound message.  It may only be delivered with
/// [`DeterministicHost::receive`].
#[derive(Clone, Debug)]
pub struct PendingEnvelope {
    meta: EnvelopeMeta,
    message: RaftMsg,
}

impl PendingEnvelope {
    pub fn meta(&self) -> &EnvelopeMeta {
        &self.meta
    }
}

fn envelope_kind(message: &RaftMsg) -> EnvelopeKind {
    match message {
        RaftMsg::Vote(_) => EnvelopeKind::Vote,
        RaftMsg::VoteResp(_) => EnvelopeKind::VoteResponse,
        RaftMsg::Append(_) => EnvelopeKind::Append,
        RaftMsg::AppendResp(_) => EnvelopeKind::AppendResponse,
        RaftMsg::InstallSnapshot(_) => EnvelopeKind::InstallSnapshot,
        RaftMsg::InstallSnapshotResp(_) => EnvelopeKind::InstallSnapshotResponse,
        RaftMsg::TimeoutNow(_) => EnvelopeKind::TimeoutNow,
    }
}

fn fingerprint(message: &RaftMsg) -> String {
    let payload = match message {
        RaftMsg::Vote(v) => serde_json::to_vec(v),
        RaftMsg::VoteResp(v) => serde_json::to_vec(v),
        RaftMsg::Append(v) => serde_json::to_vec(v),
        RaftMsg::AppendResp(v) => serde_json::to_vec(v),
        RaftMsg::InstallSnapshot(v) => serde_json::to_vec(v),
        RaftMsg::InstallSnapshotResp(v) => serde_json::to_vec(v),
        RaftMsg::TimeoutNow(v) => serde_json::to_vec(v),
    }
    .expect("raft wire messages are serializable");
    let mut hasher = Sha256::new();
    hasher.update([envelope_kind(message) as u8]);
    hasher.update(payload);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests;
