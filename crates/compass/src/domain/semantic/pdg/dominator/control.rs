use super::DominatorTree;
use crate::domain::semantic::pdg::cfg::{BlockId, ControlFlowGraph};
use std::collections::{HashMap, HashSet};

/// Control dependencies derived from post-dominator tree
#[derive(Debug, Clone)]
pub struct ControlDependencies {
    /// Control dependencies: block -> blocks it's control-dependent on
    pub dependencies: HashMap<BlockId, HashSet<BlockId>>,
    /// Reverse: block -> blocks that are control-dependent on it
    pub dependents: HashMap<BlockId, HashSet<BlockId>>,
}

impl ControlDependencies {
    /// Compute control dependencies from CFG using the Ferrante algorithm
    ///
    /// Block Y is control-dependent on block X if:
    /// 1. There exists a path from X to Y where Y post-dominates every node on the path after X
    /// 2. Y does not strictly post-dominate X
    ///
    /// Algorithm: For each edge (A → B) where B doesn't post-dominate A,
    /// walk up from B to ipdom(A), marking each node as control-dependent on A.
    pub fn compute(cfg: &ControlFlowGraph) -> Self {
        let post_dom = DominatorTree::compute_post(cfg);
        let mut dependencies: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();
        let mut dependents: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();

        // For each edge (A → B) where B does not post-dominate A
        for block_a in cfg.block_ids() {
            for block_b in cfg.get_successors(block_a) {
                // Check if B post-dominates A
                if !post_dom.dominates(block_b, block_a) {
                    // Walk up from B to ipdom(A), adding control dependencies
                    let ipdom_a = post_dom.get_idom(block_a);
                    let mut runner = block_b;
                    let mut visited = HashSet::new();

                    loop {
                        // Prevent infinite loops
                        if !visited.insert(runner) {
                            break;
                        }

                        // Runner is control-dependent on block_a (the edge source)
                        dependencies.entry(runner).or_default().insert(block_a);
                        dependents.entry(block_a).or_default().insert(runner);

                        // Stop when we reach A's immediate post-dominator
                        if Some(runner) == ipdom_a {
                            break;
                        }

                        // If ipdom(A) is None (A is exit), stop when we hit a node without ipdom
                        if ipdom_a.is_none() {
                            break;
                        }

                        // Move up the post-dominator tree
                        match post_dom.get_idom(runner) {
                            Some(dom) if dom != runner => runner = dom,
                            _ => break,
                        }
                    }
                }
            }
        }

        Self {
            dependencies,
            dependents,
        }
    }

    /// Get blocks that this block is control-dependent on
    pub fn get_dependencies(&self, block: BlockId) -> HashSet<BlockId> {
        self.dependencies.get(&block).cloned().unwrap_or_default()
    }

    /// Get blocks that are control-dependent on this block
    pub fn get_dependents(&self, block: BlockId) -> HashSet<BlockId> {
        self.dependents.get(&block).cloned().unwrap_or_default()
    }

    /// Check if block A is control-dependent on block B
    pub fn is_dependent(&self, a: BlockId, b: BlockId) -> bool {
        self.dependencies
            .get(&a)
            .map(|deps| deps.contains(&b))
            .unwrap_or(false)
    }
}
