//! State machine definition schema
//!
//! JSON schema for LLM to generate state machine definitions.
//! Designed for easy validation and Mermaid conversion.

pub use crate::domain::spec::statemachine::schema::{
    ActionDef, ActionRef, GuardDef, StateMachineDef, StateNodeDef, TransitionDetail,
    TransitionInput,
};
