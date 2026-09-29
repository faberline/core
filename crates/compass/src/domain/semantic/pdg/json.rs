use super::{PdgEdgeKind, ProgramDependenceGraph};

/// Serializable representation of PDG for MCP
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PdgJson {
    /// Nodes
    pub nodes: Vec<PdgNodeJson>,
    /// Edges
    pub edges: Vec<PdgEdgeJson>,
    /// Function name
    pub function_name: Option<String>,
    /// File path
    pub file_path: Option<String>,
    /// Statistics
    pub stats: PdgStatsJson,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PdgNodeJson {
    pub id: u32,
    pub line: usize,
    pub text: String,
    pub kind: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PdgEdgeJson {
    pub from: u32,
    pub to: u32,
    pub kind: String,
    pub variable: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PdgStatsJson {
    pub node_count: usize,
    pub control_edge_count: usize,
    pub data_edge_count: usize,
    pub total_edge_count: usize,
}

impl From<&ProgramDependenceGraph> for PdgJson {
    fn from(pdg: &ProgramDependenceGraph) -> Self {
        let nodes: Vec<PdgNodeJson> = pdg
            .all_nodes()
            .iter()
            .map(|n| PdgNodeJson {
                id: n.id,
                line: n.line,
                text: n.text.clone(),
                kind: format!("{:?}", n.kind),
            })
            .collect();

        let mut edges = Vec::new();
        for edge_list in pdg.edges.values() {
            for edge in edge_list {
                let (kind, variable) = match &edge.kind {
                    PdgEdgeKind::Control => ("control".to_string(), None),
                    PdgEdgeKind::Data { variable } => ("data".to_string(), Some(variable.clone())),
                };
                edges.push(PdgEdgeJson {
                    from: edge.from,
                    to: edge.to,
                    kind,
                    variable,
                });
            }
        }

        let stats = pdg.stats();

        Self {
            nodes,
            edges,
            function_name: pdg.function_name.clone(),
            file_path: pdg
                .file_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            stats: PdgStatsJson {
                node_count: stats.node_count,
                control_edge_count: stats.control_edge_count,
                data_edge_count: stats.data_edge_count,
                total_edge_count: stats.total_edge_count,
            },
        }
    }
}
