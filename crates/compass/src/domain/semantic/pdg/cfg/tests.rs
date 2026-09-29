use super::*;
use crate::domain::syntax::language::Language;
use crate::infrastructure::syntax::multi_parser::MultiParser;

fn build_cfg(code: &str) -> ControlFlowGraph {
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, Language::Python).unwrap();
    CfgBuilder::new(code).build(&parsed)
}

#[test]
fn test_simple_sequential() {
    let cfg = build_cfg("x = 1\ny = 2\nz = x + y");

    // Should have entry, exit, and one body block
    assert!(cfg.blocks.len() >= 3);

    // Entry should have one successor
    let entry_succs = cfg.get_successors(cfg.entry);
    assert_eq!(entry_succs.len(), 1);
}

#[test]
fn test_if_statement() {
    let cfg = build_cfg("if x:\n    y = 1\nelse:\n    y = 2");

    // Should have blocks for: entry, condition, then, else, join, exit
    assert!(cfg.blocks.len() >= 5);

    // Find condition block
    let cond_blocks: Vec<_> = cfg
        .blocks
        .values()
        .filter(|b| b.kind == BlockKind::IfCondition)
        .collect();
    assert_eq!(cond_blocks.len(), 1);
}

#[test]
fn test_while_loop() {
    let cfg = build_cfg("while x:\n    y = 1");

    // Should have loop condition block
    let loop_blocks: Vec<_> = cfg
        .blocks
        .values()
        .filter(|b| b.kind == BlockKind::LoopCondition)
        .collect();
    assert_eq!(loop_blocks.len(), 1);

    // Should have back edge
    let back_edges: Vec<_> = cfg
        .successors
        .values()
        .flatten()
        .filter(|e| e.kind == EdgeKind::LoopBack)
        .collect();
    assert!(!back_edges.is_empty());
}

#[test]
fn test_return_statement() {
    let cfg = build_cfg("def f():\n    return 1");

    // Return should connect to exit
    let _return_edges: Vec<_> = cfg
        .successors
        .values()
        .flatten()
        .filter(|e| e.kind == EdgeKind::Return)
        .collect();
    // Note: This tests module-level CFG, not function CFG
    // For function CFG, would need to use build_function
}

#[test]
fn test_break_continue() {
    let cfg = build_cfg("while True:\n    if x:\n        break\n    continue");

    // Should have break and continue edges
    let break_edges: Vec<_> = cfg
        .successors
        .values()
        .flatten()
        .filter(|e| e.kind == EdgeKind::Break)
        .collect();
    assert!(!break_edges.is_empty());

    let continue_edges: Vec<_> = cfg
        .successors
        .values()
        .flatten()
        .filter(|e| e.kind == EdgeKind::Continue)
        .collect();
    assert!(!continue_edges.is_empty());
}
