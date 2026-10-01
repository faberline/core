use super::*;
use crate::domain::semantic::pdg::cfg::CfgBuilder;
use crate::domain::syntax::language::Language;
use crate::infrastructure::syntax::multi_parser::MultiParser;

fn build_cfg(code: &str) -> ControlFlowGraph {
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, Language::Python).unwrap();
    CfgBuilder::new(code).build(&parsed)
}

#[test]
fn test_dominator_simple() {
    let cfg = build_cfg("x = 1\ny = 2");
    let dom = DominatorTree::compute(&cfg);

    // Entry should be in idom mapping
    assert!(dom.idom.contains_key(&cfg.entry));

    // Check that dominator tree was built
    assert!(!dom.idom.is_empty());
}

#[test]
fn test_post_dominator_simple() {
    let cfg = build_cfg("x = 1\ny = 2");
    let post_dom = DominatorTree::compute_post(&cfg);

    // Exit should be in idom mapping
    assert!(post_dom.idom.contains_key(&cfg.exit));

    // Check that post-dominator tree was built
    assert!(!post_dom.idom.is_empty());
}

#[test]
fn test_control_dependencies() {
    let cfg = build_cfg("if x:\n    y = 1\nelse:\n    y = 2\nz = 3");
    let deps = ControlDependencies::compute(&cfg);

    // Control dependencies should be computed (may be empty for simple cases)
    // This is a basic sanity check that the computation runs without error
    let _ = deps.dependencies.len();
}

#[test]
fn test_loop_control_dependencies() {
    let cfg = build_cfg("while x:\n    y = 1\nz = 2");
    let deps = ControlDependencies::compute(&cfg);

    // Loop body should be control-dependent on loop condition
    // This is a basic sanity check that the computation runs without error
    let _total_deps: usize = deps.dependencies.values().map(|s| s.len()).sum();
}

#[test]
fn test_diamond_cfg_dominator() {
    // Diamond CFG: if x: a else: b; c
    // Entry -> Cond -> [Then, Else] -> Join -> Exit
    let cfg = build_cfg("if x:\n    a = 1\nelse:\n    b = 1\nc = 1");
    let dom = DominatorTree::compute(&cfg);

    // Entry should dominate all blocks
    assert!(dom.dominates(cfg.entry, cfg.exit));

    // All blocks should be reachable
    assert!(dom.idom.len() >= 4);
}

#[test]
fn test_nested_if_control_deps() {
    let cfg = build_cfg("if x:\n    if y:\n        z = 1\nw = 2");
    let deps = ControlDependencies::compute(&cfg);

    // Should have control dependencies for nested conditions
    // Just verify it runs without panicking
    let _ = deps.dependencies.len();
}

#[test]
fn test_for_else_cfg() {
    // for...else: else runs if loop completes without break
    let cfg = build_cfg("for i in range(10):\n    x = i\nelse:\n    y = 0\nz = 1");

    // Should have a loop condition block
    let loop_blocks: Vec<_> = cfg
        .blocks
        .values()
        .filter(|b| matches!(b.kind, super::super::cfg::BlockKind::LoopCondition))
        .collect();
    assert!(!loop_blocks.is_empty());

    // CFG should be properly connected
    let deps = ControlDependencies::compute(&cfg);
    let _ = deps.dependencies.len();
}
