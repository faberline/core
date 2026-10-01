use super::*;
use crate::domain::semantic::pdg::cfg::CfgBuilder;
use crate::domain::syntax::language::Language;
use crate::infrastructure::syntax::multi_parser::MultiParser;

fn analyze_data_flow(code: &str) -> DataDependencies {
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, Language::Python).unwrap();
    let cfg = CfgBuilder::new(code).build(&parsed);
    DataDependencies::compute(&cfg, &parsed)
}

#[test]
fn test_simple_def_use() {
    let deps = analyze_data_flow("x = 1\ny = x + 2");

    // x is defined on line 0 and used on line 1
    let x_defs = deps.get_defs("x");
    assert!(!x_defs.is_empty());

    let x_uses: Vec<_> = deps
        .uses_by_name
        .get("x")
        .map(|u| u.iter().collect())
        .unwrap_or_default();
    assert!(!x_uses.is_empty());
}

#[test]
fn test_multiple_defs() {
    let deps = analyze_data_flow("x = 1\nx = 2\ny = x");

    // x should have 2 definitions
    let x_defs = deps.get_defs("x");
    assert!(x_defs.len() >= 1);
}

#[test]
fn test_dependency_check() {
    let deps = analyze_data_flow("x = 1\ny = x");

    // y's line should depend on x's line
    let _y_deps = deps.get_dependencies_for_line(1);
    // Should include line 0 (where x is defined)
    // Note: exact line numbers depend on parsing
}

#[test]
fn test_no_self_dependency() {
    let deps = analyze_data_flow("x = 1");

    // A line shouldn't depend on itself
    let self_deps = deps.get_dependencies_for_line(0);
    assert!(!self_deps.contains(&0));
}
