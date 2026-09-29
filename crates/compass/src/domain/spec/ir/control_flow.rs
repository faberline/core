//! Control flow IR (from Mermaid flowchart).

// ============================================================================
// Control Flow Specification (from Mermaid flowchart)
// ============================================================================

/// Control flow specification
#[derive(Debug, Clone, Default)]
pub struct ControlFlowSpec {
    /// Flow name
    pub name: String,
    /// Nodes
    pub nodes: Vec<FlowNode>,
    /// Edges
    pub edges: Vec<FlowEdge>,
}

/// Flow node types
#[derive(Debug, Clone)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    pub node_type: FlowNodeType,
}

/// Flow node type
#[derive(Debug, Clone)]
pub enum FlowNodeType {
    Start,
    End,
    Process,
    Decision,
    SubProcess,
    InputOutput,
    Database,
}

/// Flow edge
#[derive(Debug, Clone)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    /// For decision nodes: true/false branch
    pub condition: Option<bool>,
}
