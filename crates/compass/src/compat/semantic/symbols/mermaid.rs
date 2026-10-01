//! Mermaid diagram symbol extraction (line-based)
//!
//! Extracts symbols from Mermaid diagram source:
//! - Diagram type (first non-empty line) as Module
//! - Node IDs in flowchart/graph as Variable
//! - Edge labels (`A -->|label| B`) as Label
//! - Subgraph names (`subgraph Name`) as Module
