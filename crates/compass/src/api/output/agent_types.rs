//! Serde-serializable types for agent output format.
//!
//! Symbol-centric JSON output optimized for LLM agent consumption.
//! Uses `skip_serializing_if` to omit empty fields for compactness (R9).

pub use crate::interfaces::output::agent_types::{AgentIssue, AgentOutput, AgentStats, SymbolDef};
