use super::{PdgEdgeKind, ProgramDependenceGraph};

impl ProgramDependenceGraph {
    /// Get statistics about the PDG
    pub fn stats(&self) -> PdgStats {
        let mut control_edges = 0;
        let mut data_edges = 0;

        for edges in self.edges.values() {
            for edge in edges {
                match edge.kind {
                    PdgEdgeKind::Control => control_edges += 1,
                    PdgEdgeKind::Data { .. } => data_edges += 1,
                }
            }
        }

        PdgStats {
            node_count: self.nodes.len(),
            control_edge_count: control_edges,
            data_edge_count: data_edges,
            total_edge_count: control_edges + data_edges,
        }
    }
}

/// Statistics about a PDG
#[derive(Debug, Clone)]
pub struct PdgStats {
    /// Number of nodes
    pub node_count: usize,
    /// Number of control dependency edges
    pub control_edge_count: usize,
    /// Number of data dependency edges
    pub data_edge_count: usize,
    /// Total number of edges
    pub total_edge_count: usize,
}
