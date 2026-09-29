use super::ProgramDependenceGraph;
use std::collections::HashSet;

impl ProgramDependenceGraph {
    /// Detect dead code (statements not affecting any output)
    ///
    /// A statement is dead if:
    /// 1. It has no dependents (forward edges), OR
    /// 2. All its dependents are also dead
    pub fn dead_code_detection(&self, output_lines: &[usize]) -> DeadCodeAnalysis {
        let mut analysis = DeadCodeAnalysis::new();

        // Find all lines that affect outputs (backward slice from outputs)
        let mut live = HashSet::new();
        for &output in output_lines {
            let slice = self.backward_slice(output);
            for node in slice.nodes {
                live.insert(node.line);
            }
        }

        // All lines not in live set are dead
        for node in self.nodes.values() {
            if !live.contains(&node.line) {
                analysis.dead_lines.push(node.line);
            }
        }

        analysis.dead_lines.sort();
        analysis.dead_lines.dedup();
        analysis
    }
}

/// Result of dead code detection
#[derive(Debug, Clone)]
pub struct DeadCodeAnalysis {
    /// Lines that are dead (don't affect outputs)
    pub dead_lines: Vec<usize>,
}

impl DeadCodeAnalysis {
    fn new() -> Self {
        Self {
            dead_lines: Vec::new(),
        }
    }

    /// Check if a line is dead
    pub fn is_dead(&self, line: usize) -> bool {
        self.dead_lines.contains(&line)
    }

    /// Get number of dead lines
    pub fn dead_count(&self) -> usize {
        self.dead_lines.len()
    }
}
