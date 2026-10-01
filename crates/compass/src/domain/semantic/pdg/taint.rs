use super::ProgramDependenceGraph;
use std::collections::{HashMap, HashSet, VecDeque};

impl ProgramDependenceGraph {
    /// Perform taint tracking from sources to sinks
    ///
    /// Returns all paths where tainted data flows from sources to sinks.
    pub fn taint_tracking(&self, sources: &[usize], sinks: &[usize]) -> TaintAnalysis {
        let mut analysis = TaintAnalysis::new();
        analysis.sources = sources.to_vec();
        analysis.sinks = sinks.to_vec();

        // Get all nodes reachable from sources (forward slice)
        let mut tainted = HashSet::new();
        for &source in sources {
            let slice = self.forward_slice(source);
            for node in slice.nodes {
                tainted.insert(node.line);
            }
        }

        // Find sinks that are tainted
        for &sink in sinks {
            if tainted.contains(&sink) {
                // Find the path from source to sink
                for &source in sources {
                    if let Some(path) = self.find_path(source, sink) {
                        analysis.taint_paths.push(TaintPath { source, sink, path });
                    }
                }
            }
        }

        analysis
    }

    /// Find a path from one line to another
    fn find_path(&self, from_line: usize, to_line: usize) -> Option<Vec<usize>> {
        // Get first node on each line (could extend to find path through any node)
        let from_id = *self.line_to_nodes.get(&from_line)?.first()?;
        let to_id = *self.line_to_nodes.get(&to_line)?.first()?;

        // BFS to find path
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent: HashMap<u32, u32> = HashMap::new();

        queue.push_back(from_id);
        visited.insert(from_id);

        while let Some(current) = queue.pop_front() {
            if current == to_id {
                // Reconstruct path
                let mut path = Vec::new();
                let mut node = to_id;
                while node != from_id {
                    if let Some(n) = self.nodes.get(&node) {
                        path.push(n.line);
                    }
                    node = *parent.get(&node)?;
                }
                if let Some(n) = self.nodes.get(&from_id) {
                    path.push(n.line);
                }
                path.reverse();
                return Some(path);
            }

            if let Some(edges) = self.edges.get(&current) {
                for edge in edges {
                    if visited.insert(edge.to) {
                        parent.insert(edge.to, current);
                        queue.push_back(edge.to);
                    }
                }
            }
        }

        None
    }
}

/// A path from taint source to sink
#[derive(Debug, Clone)]
pub struct TaintPath {
    /// Source line
    pub source: usize,
    /// Sink line
    pub sink: usize,
    /// Path of lines from source to sink
    pub path: Vec<usize>,
}

/// Result of taint analysis
#[derive(Debug, Clone)]
pub struct TaintAnalysis {
    /// Taint sources
    pub sources: Vec<usize>,
    /// Taint sinks
    pub sinks: Vec<usize>,
    /// Paths from sources to sinks
    pub taint_paths: Vec<TaintPath>,
}

impl TaintAnalysis {
    fn new() -> Self {
        Self {
            sources: Vec::new(),
            sinks: Vec::new(),
            taint_paths: Vec::new(),
        }
    }

    /// Check if any sink is tainted
    pub fn has_taint(&self) -> bool {
        !self.taint_paths.is_empty()
    }

    /// Get all tainted sinks
    pub fn tainted_sinks(&self) -> Vec<usize> {
        self.taint_paths.iter().map(|p| p.sink).collect()
    }
}
