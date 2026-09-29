//! Mermaid diagram parser
//!
//! Parses Mermaid diagrams into SpecIR structures.

use crate::domain::spec::ir::{ControlFlowSpec, DataModelSpec, StateMachineSpec};

mod class_diagram;
mod er_diagram;
mod flowchart;
mod state_diagram;

/// Error type for Mermaid parsing
#[derive(Debug)]
pub enum MermaidError {
    /// Unknown diagram type
    UnknownDiagramType(String),
    /// Invalid syntax
    SyntaxError(String),
    /// Missing required element
    MissingElement(String),
    /// Other error
    Other(String),
}

impl std::fmt::Display for MermaidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MermaidError::UnknownDiagramType(s) => write!(f, "unknown diagram type: {}", s),
            MermaidError::SyntaxError(s) => write!(f, "syntax error: {}", s),
            MermaidError::MissingElement(s) => write!(f, "missing element: {}", s),
            MermaidError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for MermaidError {}

/// Detected diagram type
#[derive(Debug, Clone, PartialEq)]
pub enum DiagramType {
    ClassDiagram,
    SequenceDiagram,
    StateDiagram,
    Flowchart,
    ErDiagram,
}

/// Parsed Mermaid result
#[derive(Debug)]
pub enum MermaidSpec {
    DataModel(DataModelSpec),
    StateMachine(StateMachineSpec),
    ControlFlow(ControlFlowSpec),
}

/// Mermaid diagram parser
pub struct MermaidParser;

impl MermaidParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse Mermaid diagram and detect type automatically
    pub fn parse(&self, content: &str) -> Result<MermaidSpec, MermaidError> {
        let content = content.trim();
        let diagram_type = self.detect_diagram_type(content)?;

        match diagram_type {
            DiagramType::ClassDiagram => {
                let spec = self.parse_class_diagram(content)?;
                Ok(MermaidSpec::DataModel(spec))
            }
            DiagramType::StateDiagram => {
                let spec = self.parse_state_diagram(content)?;
                Ok(MermaidSpec::StateMachine(spec))
            }
            DiagramType::Flowchart => {
                let spec = self.parse_flowchart(content)?;
                Ok(MermaidSpec::ControlFlow(spec))
            }
            DiagramType::ErDiagram => {
                let spec = self.parse_er_diagram(content)?;
                Ok(MermaidSpec::DataModel(spec))
            }
            DiagramType::SequenceDiagram => {
                // Sequence diagrams don't map directly to our IR
                // Return empty data model for now
                Ok(MermaidSpec::DataModel(DataModelSpec::default()))
            }
        }
    }

    /// Detect diagram type from content
    fn detect_diagram_type(&self, content: &str) -> Result<DiagramType, MermaidError> {
        let first_line = content.lines().next().unwrap_or("").trim().to_lowercase();

        if first_line.starts_with("classdiagram") {
            Ok(DiagramType::ClassDiagram)
        } else if first_line.starts_with("sequencediagram") {
            Ok(DiagramType::SequenceDiagram)
        } else if first_line.starts_with("statediagram") {
            Ok(DiagramType::StateDiagram)
        } else if first_line.starts_with("flowchart") || first_line.starts_with("graph") {
            Ok(DiagramType::Flowchart)
        } else if first_line.starts_with("erdiagram") {
            Ok(DiagramType::ErDiagram)
        } else {
            Err(MermaidError::UnknownDiagramType(first_line))
        }
    }
}

impl Default for MermaidParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
