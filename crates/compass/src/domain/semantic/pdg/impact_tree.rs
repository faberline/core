use super::{PdgEdgeKind, ProgramDependenceGraph};
use std::collections::HashSet;

// ============================================================================
// Dependency tree for impact analysis (R6)
// ============================================================================

/// Kind of dependency in the impact tree
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyReason {
    Data,
    Control,
    Transitive,
}

/// A node in the impact dependency tree
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImpactTreeNode {
    pub line: usize,
    pub text: String,
    pub reason: DependencyReason,
    pub variable: Option<String>,
    pub children: Vec<ImpactTreeNode>,
}

/// Impact analysis with dependency tree output (R6)
#[derive(Debug, Clone)]
pub struct ImpactAnalysisTree {
    pub changed_lines: Vec<usize>,
    pub affected_lines: Vec<usize>,
    pub tree: Vec<ImpactTreeNode>,
}

impl ProgramDependenceGraph {
    /// Compute impact analysis with dependency tree format
    ///
    /// Returns WHY each line is affected (data dep, control dep, transitive)
    /// as a tree structure suitable for AI agent consumption.
    pub fn impact_analysis_tree(&self, changed_lines: &[usize]) -> ImpactAnalysisTree {
        let mut all_affected = Vec::new();
        let mut tree_roots = Vec::new();

        for &line in changed_lines {
            let mut visited = HashSet::new();
            let mut root_children = Vec::new();
            self.collect_impact_tree(line, &mut root_children, &mut visited, 0);
            tree_roots.extend(root_children);

            // Collect flat list
            let flat: Vec<usize> = self
                .forward_slice(line)
                .nodes
                .iter()
                .filter(|n| n.line != line)
                .map(|n| n.line)
                .collect();
            all_affected.extend(flat);
        }

        all_affected.sort();
        all_affected.dedup();

        ImpactAnalysisTree {
            changed_lines: changed_lines.to_vec(),
            affected_lines: all_affected,
            tree: tree_roots,
        }
    }

    fn collect_impact_tree(
        &self,
        node_id_or_line: usize,
        children: &mut Vec<ImpactTreeNode>,
        visited: &mut HashSet<usize>,
        depth: usize,
    ) {
        // Guard against deep recursion
        if depth > 20 {
            return;
        }

        let node_ids = match self.line_to_nodes.get(&node_id_or_line) {
            Some(ids) => ids.clone(),
            None => return,
        };

        for &node_id in &node_ids {
            if let Some(edges) = self.edges.get(&node_id) {
                for edge in edges {
                    if let Some(target) = self.nodes.get(&edge.to) {
                        if !visited.insert(target.line) {
                            continue;
                        }

                        let (reason, variable) = match &edge.kind {
                            PdgEdgeKind::Data { variable } => {
                                (DependencyReason::Data, Some(variable.clone()))
                            }
                            PdgEdgeKind::Control => (DependencyReason::Control, None),
                        };

                        let mut node_children = Vec::new();
                        self.collect_impact_tree(
                            target.line,
                            &mut node_children,
                            visited,
                            depth + 1,
                        );

                        // Mark transitive if it has children
                        let effective_reason = if !node_children.is_empty() && depth > 0 {
                            DependencyReason::Transitive
                        } else {
                            reason
                        };

                        children.push(ImpactTreeNode {
                            line: target.line,
                            text: target.text.clone(),
                            reason: effective_reason,
                            variable,
                            children: node_children,
                        });
                    }
                }
            }
        }
    }
}
