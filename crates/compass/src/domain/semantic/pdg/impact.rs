use super::ProgramDependenceGraph;
use std::collections::HashSet;

impl ProgramDependenceGraph {
    /// Compute impact analysis for a set of changed lines
    ///
    /// Returns all lines that may be affected by changes.
    pub fn impact_analysis(&self, changed_lines: &[usize]) -> ImpactAnalysis {
        let mut impact = ImpactAnalysis::new();
        impact.changed_lines = changed_lines.to_vec();

        let mut affected = HashSet::new();

        for &line in changed_lines {
            let slice = self.forward_slice(line);
            for node in slice.nodes {
                if node.line != line {
                    affected.insert(node.line);
                }
            }
        }

        impact.affected_lines = affected.into_iter().collect();
        impact.affected_lines.sort();
        impact
    }
}

/// Result of impact analysis
#[derive(Debug, Clone)]
pub struct ImpactAnalysis {
    /// Lines that were changed
    pub changed_lines: Vec<usize>,
    /// Lines affected by the changes
    pub affected_lines: Vec<usize>,
}

impl ImpactAnalysis {
    fn new() -> Self {
        Self {
            changed_lines: Vec::new(),
            affected_lines: Vec::new(),
        }
    }

    /// Get total impact (number of affected lines)
    pub fn impact_count(&self) -> usize {
        self.affected_lines.len()
    }
}
