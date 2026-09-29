//! Mermaid+ generator
//!
//! Generates Mermaid+ output from validated state machine definitions.
//! Mermaid+ = YAML frontmatter (structured definition) + Mermaid diagram

pub use crate::domain::spec::statemachine::mermaid_plus::{
    MermaidPlusGenerator, MermaidPlusOutput,
};
