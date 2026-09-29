//! State machine definition parsing, validation, and Mermaid+ generation
//!
//! Flow:
//! 1. LLM generates structured JSON (state machine definition)
//! 2. Lens validates the JSON semantically
//! 3. Lens outputs Mermaid+ (YAML frontmatter + Mermaid diagram)
//!
//! The JSON schema is designed for:
//! - Easy generation by LLM
//! - Easy validation by code
//! - Conversion to Mermaid stateDiagram-v2

pub(crate) mod mermaid_plus;
pub(crate) mod schema;
pub(crate) mod validator;
