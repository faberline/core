//! `cclab.llm.v2` — offline task navigation for agent-facing CLIs.
//!
//! Version 1 remains the lightweight `Topic` registry. Version 2 adds typed
//! task selection and runbooks while preserving the `topic` and `markdown`
//! JSON compatibility fields consumed by existing clients.

pub use crate::application::llm::v2::ProtocolDocument;
pub use crate::domain::llm::v2::{
    json_schema, Input, ProviderContent, Risk, Runbook, Step, Task, Topic, PROTOCOL,
};
