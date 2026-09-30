//! Agent output builder: constructs symbol-centric JSON from analysis results.
//!
//! Combines SymbolTable, ImportGraph, and lint diagnostics into a compact
//! representation optimized for LLM agent consumption.

pub use crate::interfaces::output::agent::AgentOutputBuilder;
