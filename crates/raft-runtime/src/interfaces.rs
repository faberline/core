//! Interface layer: the peer HTTP handlers and routers, the
//! deterministic conformance host, the cluster-state view and the LLM topic.

mod conformance;
mod llm;
pub(crate) mod peer_http;
mod view;

pub use conformance::{
    ConformanceMembershipError, ConformanceRole, DeterministicHost, EnvelopeKind, EnvelopeMeta,
    NodeView, PendingEnvelope, StateMachineOperation, StepError, TRACE_SCHEMA,
};
pub use llm::{topic, TOPIC};
pub use view::{ClusterStateView, PeerAddr, RaftRole};
