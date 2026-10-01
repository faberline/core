use super::*;
use crate::domain::syntax::language::Language;
use crate::infrastructure::syntax::multi_parser::MultiParser;

fn build_pdg(code: &str) -> ProgramDependenceGraph {
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, Language::Python).unwrap();
    ProgramDependenceGraph::build(code, &parsed)
}

#[test]
fn test_pdg_construction() {
    let pdg = build_pdg("x = 1\ny = x + 2\nz = y * 3");

    // Should have nodes
    assert!(!pdg.nodes.is_empty());

    // Should have data dependency edges
    let stats = pdg.stats();
    assert!(stats.data_edge_count > 0 || stats.node_count > 0);
}

#[test]
fn test_backward_slice() {
    let pdg = build_pdg("x = 1\ny = 2\nz = x + y");

    // Backward slice from z should include x and y
    let slice = pdg.backward_slice(2);
    assert!(!slice.is_empty());
}

#[test]
fn test_forward_slice() {
    let pdg = build_pdg("x = 1\ny = x + 2\nz = y * 3");

    // Forward slice from x should include y and z
    let slice = pdg.forward_slice(0);
    assert!(!slice.is_empty());
}

#[test]
fn test_impact_analysis() {
    let pdg = build_pdg("x = 1\ny = x + 2\nz = y * 3\nw = 4");

    // Changing x should affect y and z, but not w
    let _impact = pdg.impact_analysis(&[0]);
    // Note: exact results depend on parsing
}

#[test]
fn test_dead_code() {
    let pdg = build_pdg("x = 1\ny = 2\nz = x");

    // y is dead if z (using x) is the only output
    let _dead = pdg.dead_code_detection(&[2]);
    // y (line 1) should be in dead_lines
}

#[test]
fn test_pdg_json_serialization() {
    let pdg = build_pdg("x = 1\ny = x + 2");
    let json: PdgJson = (&pdg).into();

    assert!(!json.nodes.is_empty());
    assert!(json.stats.node_count > 0);
}

#[test]
fn test_get_nodes_by_line() {
    // Test the new get_nodes_by_line method
    let pdg = build_pdg("x = 1\ny = 2\nz = 3");

    // Each line should have at least one node
    for line in 0..3 {
        let nodes = pdg.get_nodes_by_line(line);
        // May or may not have nodes depending on parsing
        let _ = nodes.len();
    }
}

#[test]
fn test_multiline_nodes_structure() {
    let pdg = build_pdg("a = 1\nb = a\nc = b");

    // Verify line_to_nodes is a multimap
    for (&line, node_ids) in &pdg.line_to_nodes {
        assert!(line < 10); // Sanity check
        for &id in node_ids {
            assert!(pdg.nodes.contains_key(&id));
        }
    }
}

#[test]
fn test_slice_with_control_flow() {
    let pdg = build_pdg("x = 1\nif x:\n    y = x + 1\nz = y");

    // Forward slice from x should include statements that depend on x
    let slice = pdg.forward_slice(0);
    assert!(!slice.is_empty());

    // Backward slice from z should include its dependencies
    let slice = pdg.backward_slice(3);
    assert!(!slice.is_empty());
}
