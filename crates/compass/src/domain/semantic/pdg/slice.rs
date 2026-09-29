use super::{PdgNode, ProgramDependenceGraph};
use std::collections::HashSet;

impl ProgramDependenceGraph {
    /// Compute backward slice from a criterion (line number)
    ///
    /// Returns all statements that affect the given line.
    /// If multiple statements exist on the same line, includes all of them.
    pub fn backward_slice(&self, line: usize) -> ProgramSlice {
        let mut slice = ProgramSlice::new(SliceDirection::Backward);

        if let Some(node_ids) = self.line_to_nodes.get(&line) {
            slice.criterion_line = Some(line);
            let mut visited = HashSet::new();
            for &node_id in node_ids {
                self.collect_backward_slice(node_id, &mut slice.nodes, &mut visited);
            }
        }

        // Sort by line number and deduplicate
        slice.nodes.sort_by_key(|n| (n.line, n.id));
        slice.nodes.dedup_by_key(|n| n.id);
        slice
    }

    fn collect_backward_slice(
        &self,
        node_id: u32,
        result: &mut Vec<PdgNode>,
        visited: &mut HashSet<u32>,
    ) {
        if !visited.insert(node_id) {
            return;
        }

        if let Some(node) = self.nodes.get(&node_id) {
            result.push(node.clone());

            // Follow backward edges (dependencies)
            if let Some(edges) = self.reverse_edges.get(&node_id) {
                for edge in edges {
                    self.collect_backward_slice(edge.from, result, visited);
                }
            }
        }
    }

    /// Compute forward slice from a criterion (line number)
    ///
    /// Returns all statements affected by the given line.
    /// If multiple statements exist on the same line, includes all of them.
    pub fn forward_slice(&self, line: usize) -> ProgramSlice {
        let mut slice = ProgramSlice::new(SliceDirection::Forward);

        if let Some(node_ids) = self.line_to_nodes.get(&line) {
            slice.criterion_line = Some(line);
            let mut visited = HashSet::new();
            for &node_id in node_ids {
                self.collect_forward_slice(node_id, &mut slice.nodes, &mut visited);
            }
        }

        // Sort by line number and deduplicate
        slice.nodes.sort_by_key(|n| (n.line, n.id));
        slice.nodes.dedup_by_key(|n| n.id);
        slice
    }

    fn collect_forward_slice(
        &self,
        node_id: u32,
        result: &mut Vec<PdgNode>,
        visited: &mut HashSet<u32>,
    ) {
        if !visited.insert(node_id) {
            return;
        }

        if let Some(node) = self.nodes.get(&node_id) {
            result.push(node.clone());

            // Follow forward edges (dependents)
            if let Some(edges) = self.edges.get(&node_id) {
                for edge in edges {
                    self.collect_forward_slice(edge.to, result, visited);
                }
            }
        }
    }
}

/// Direction of program slice
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceDirection {
    Forward,
    Backward,
}

/// Result of program slicing
#[derive(Debug, Clone)]
pub struct ProgramSlice {
    /// Direction of the slice
    pub direction: SliceDirection,
    /// Criterion line (if applicable)
    pub criterion_line: Option<usize>,
    /// Nodes in the slice
    pub nodes: Vec<PdgNode>,
}

impl ProgramSlice {
    fn new(direction: SliceDirection) -> Self {
        Self {
            direction,
            criterion_line: None,
            nodes: Vec::new(),
        }
    }

    /// Get lines in the slice
    pub fn lines(&self) -> Vec<usize> {
        self.nodes.iter().map(|n| n.line).collect()
    }

    /// Check if a line is in the slice
    pub fn contains_line(&self, line: usize) -> bool {
        self.nodes.iter().any(|n| n.line == line)
    }

    /// Get the number of statements in the slice
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the slice is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
