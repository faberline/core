//! Deterministic, in-process host for adversarial Raft recovery tests.
//!
//! This module deliberately has no runtime, socket, timer, or wall-clock
//! dependency.  A harness drives logical time with [`DeterministicHost::tick`]
//! and moves opaque envelopes itself.  It uses the production host's cold-start,
//! apply, persistence, and peer-lane primitives so its result is evidence about
//! the shipped driver rather than a second implementation of it.

pub use crate::interfaces::{
    ConformanceMembershipError, ConformanceRole, DeterministicHost, EnvelopeKind, EnvelopeMeta,
    NodeView, PendingEnvelope, StateMachineOperation, StepError, TRACE_SCHEMA,
};
