//! Dominator and Post-Dominator Tree Analysis
//!
//! Implements dominator tree construction for control dependency analysis.
//! Uses Cooper's algorithm (simpler than Lengauer-Tarjan, efficient for small graphs).

mod control;
#[cfg(test)]
mod tests;

use super::cfg::{BlockId, ControlFlowGraph};
use std::collections::{HashMap, HashSet};

pub use control::ControlDependencies;

/// Dominator tree for a CFG
#[derive(Debug, Clone)]
pub struct DominatorTree {
    /// Immediate dominator for each block
    pub idom: HashMap<BlockId, BlockId>,
    /// Dominance frontier for each block
    pub frontier: HashMap<BlockId, HashSet<BlockId>>,
    /// Children in dominator tree
    pub children: HashMap<BlockId, Vec<BlockId>>,
    /// Root of the tree (entry for dominators, exit for post-dominators)
    pub root: BlockId,
}

impl DominatorTree {
    /// Compute dominator tree from CFG
    pub fn compute(cfg: &ControlFlowGraph) -> Self {
        // For dominators: traverse successors for RPO, use predecessors for algorithm
        let idom = Self::compute_idom(
            cfg,
            cfg.entry,
            |cfg, b| cfg.get_successors(b),   // For RPO traversal
            |cfg, b| cfg.get_predecessors(b), // For predecessor lookup in algorithm
        );
        let children = Self::build_children(&idom);
        let frontier = Self::compute_frontier(cfg, &idom, |cfg, b| cfg.get_successors(b));

        Self {
            idom,
            frontier,
            children,
            root: cfg.entry,
        }
    }

    /// Compute post-dominator tree from CFG
    pub fn compute_post(cfg: &ControlFlowGraph) -> Self {
        // For post-dominators: traverse predecessors for RPO (reverse graph), use successors
        let idom = Self::compute_idom(
            cfg,
            cfg.exit,
            |cfg, b| cfg.get_predecessors(b), // For RPO traversal (reverse graph)
            |cfg, b| cfg.get_successors(b),   // For "predecessor" lookup (successors in reverse)
        );
        let children = Self::build_children(&idom);
        let frontier = Self::compute_frontier(cfg, &idom, |cfg, b| cfg.get_predecessors(b));

        Self {
            idom,
            frontier,
            children,
            root: cfg.exit,
        }
    }

    /// Compute immediate dominators using Cooper's algorithm
    /// (simpler than Lengauer-Tarjan but efficient for small graphs)
    ///
    /// Parameters:
    /// - get_succs: Used for RPO traversal (successors from root)
    /// - get_preds: Used for predecessor lookup in the algorithm
    fn compute_idom<F, G>(
        cfg: &ControlFlowGraph,
        root: BlockId,
        get_succs: F,
        get_preds: G,
    ) -> HashMap<BlockId, BlockId>
    where
        F: Fn(&ControlFlowGraph, BlockId) -> Vec<BlockId>,
        G: Fn(&ControlFlowGraph, BlockId) -> Vec<BlockId>,
    {
        let mut idom: HashMap<BlockId, BlockId> = HashMap::new();

        // Initialize: root dominates itself
        idom.insert(root, root);

        // Compute reverse post-order traversal using successors
        let rpo = Self::reverse_post_order(cfg, root, &get_succs);
        let rpo_number: HashMap<BlockId, usize> =
            rpo.iter().enumerate().map(|(i, &b)| (b, i)).collect();

        // Iterate until fixed point
        let mut changed = true;
        while changed {
            changed = false;

            for &block in &rpo {
                if block == root {
                    continue;
                }

                let preds = get_preds(cfg, block);
                let mut new_idom: Option<BlockId> = None;

                for pred in preds {
                    if !idom.contains_key(&pred) {
                        continue;
                    }

                    new_idom = match new_idom {
                        None => Some(pred),
                        Some(current) => Some(Self::intersect(&idom, &rpo_number, current, pred)),
                    };
                }

                if let Some(dom) = new_idom {
                    if idom.get(&block) != Some(&dom) {
                        idom.insert(block, dom);
                        changed = true;
                    }
                }
            }
        }

        idom
    }

    /// Intersect two dominators (find lowest common ancestor in dominator tree)
    /// Uses the standard Cooper algorithm intersect
    fn intersect(
        idom: &HashMap<BlockId, BlockId>,
        rpo_number: &HashMap<BlockId, usize>,
        mut b1: BlockId,
        mut b2: BlockId,
    ) -> BlockId {
        // Maximum iterations to prevent infinite loops
        let max_iters = idom.len() + 1;
        let mut iters = 0;

        while b1 != b2 && iters < max_iters {
            iters += 1;

            let mut n1 = rpo_number.get(&b1).copied().unwrap_or(usize::MAX);
            let mut n2 = rpo_number.get(&b2).copied().unwrap_or(usize::MAX);

            while n1 > n2 {
                let new_b1 = *idom.get(&b1).unwrap_or(&b1);
                if new_b1 == b1 {
                    break; // Reached root
                }
                b1 = new_b1;
                n1 = rpo_number.get(&b1).copied().unwrap_or(usize::MAX);
            }

            while n2 > n1 {
                let new_b2 = *idom.get(&b2).unwrap_or(&b2);
                if new_b2 == b2 {
                    break; // Reached root
                }
                b2 = new_b2;
                n2 = rpo_number.get(&b2).copied().unwrap_or(usize::MAX);
            }
        }
        b1
    }

    /// Compute reverse post-order traversal
    fn reverse_post_order<F>(cfg: &ControlFlowGraph, root: BlockId, get_succs: &F) -> Vec<BlockId>
    where
        F: Fn(&ControlFlowGraph, BlockId) -> Vec<BlockId>,
    {
        let mut visited = HashSet::new();
        let mut post_order = Vec::new();

        Self::dfs_post_order(cfg, root, &mut visited, &mut post_order, get_succs);

        post_order.reverse();
        post_order
    }

    /// DFS for post-order traversal
    fn dfs_post_order<F>(
        cfg: &ControlFlowGraph,
        block: BlockId,
        visited: &mut HashSet<BlockId>,
        post_order: &mut Vec<BlockId>,
        get_succs: &F,
    ) where
        F: Fn(&ControlFlowGraph, BlockId) -> Vec<BlockId>,
    {
        if !visited.insert(block) {
            return;
        }

        for succ in get_succs(cfg, block) {
            Self::dfs_post_order(cfg, succ, visited, post_order, get_succs);
        }

        post_order.push(block);
    }

    /// Build children map from idom
    fn build_children(idom: &HashMap<BlockId, BlockId>) -> HashMap<BlockId, Vec<BlockId>> {
        let mut children: HashMap<BlockId, Vec<BlockId>> = HashMap::new();

        for (&block, &dom) in idom {
            if block != dom {
                children.entry(dom).or_default().push(block);
            }
        }

        children
    }

    /// Compute dominance frontier
    fn compute_frontier<F>(
        cfg: &ControlFlowGraph,
        idom: &HashMap<BlockId, BlockId>,
        get_succs: F,
    ) -> HashMap<BlockId, HashSet<BlockId>>
    where
        F: Fn(&ControlFlowGraph, BlockId) -> Vec<BlockId>,
    {
        let mut frontier: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();

        for &block in idom.keys() {
            let succs = get_succs(cfg, block);

            for succ in succs {
                let mut runner = block;

                // Walk up the dominator tree until we dominate succ
                while runner != *idom.get(&succ).unwrap_or(&succ) && runner != succ {
                    frontier.entry(runner).or_default().insert(succ);

                    if let Some(&dom) = idom.get(&runner) {
                        if dom == runner {
                            break;
                        }
                        runner = dom;
                    } else {
                        break;
                    }
                }
            }
        }

        frontier
    }

    /// Check if block A dominates block B
    pub fn dominates(&self, a: BlockId, b: BlockId) -> bool {
        if a == b {
            return true;
        }

        let mut current = b;
        while let Some(&dom) = self.idom.get(&current) {
            if dom == a {
                return true;
            }
            if dom == current {
                break;
            }
            current = dom;
        }

        false
    }

    /// Get all blocks dominated by a given block
    pub fn dominated_by(&self, block: BlockId) -> HashSet<BlockId> {
        let mut result = HashSet::new();
        self.collect_dominated(block, &mut result);
        result
    }

    fn collect_dominated(&self, block: BlockId, result: &mut HashSet<BlockId>) {
        result.insert(block);
        if let Some(children) = self.children.get(&block) {
            for &child in children {
                self.collect_dominated(child, result);
            }
        }
    }

    /// Get immediate dominator of a block
    pub fn get_idom(&self, block: BlockId) -> Option<BlockId> {
        self.idom.get(&block).copied().filter(|&d| d != block)
    }

    /// Get dominance frontier of a block
    pub fn get_frontier(&self, block: BlockId) -> HashSet<BlockId> {
        self.frontier.get(&block).cloned().unwrap_or_default()
    }
}
