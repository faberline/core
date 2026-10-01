use super::{super::checker::Checker, MermaidChecker};
use crate::checker::LintConfig;
use crate::syntax::ParsedFile;

fn make_file(source: &str) -> ParsedFile {
    ParsedFile::line_based(source.to_string(), crate::syntax::Language::Mermaid)
}

fn check(source: &str) -> Vec<String> {
    let checker = MermaidChecker::new();
    let file = make_file(source);
    let config = LintConfig::default();
    checker
        .check(&file, &config)
        .iter()
        .map(|d| d.code.clone())
        .collect()
}

#[test]
fn test_valid_flowchart_no_errors() {
    let src = "flowchart LR\n    A[Start] --> B[End]\n";
    let codes = check(src);
    assert!(
        codes.is_empty(),
        "expected no diagnostics, got: {:?}",
        codes
    );
}

#[test]
fn test_mm001_unknown_diagram_type() {
    let src = "invalidType\n    A --> B\n";
    let codes = check(src);
    assert!(
        codes.contains(&"MM001".to_string()),
        "expected MM001, got: {:?}",
        codes
    );
}

#[test]
fn test_mm004_empty_diagram() {
    let src = "flowchart LR\n";
    let codes = check(src);
    assert!(
        codes.contains(&"MM004".to_string()),
        "expected MM004, got: {:?}",
        codes
    );
}

#[test]
fn test_mm003_duplicate_node() {
    let src = "graph TD\n    A[First]\n    A[Second]\n    A --> B[End]\n";
    let codes = check(src);
    assert!(
        codes.contains(&"MM003".to_string()),
        "expected MM003, got: {:?}",
        codes
    );
}

#[test]
fn test_mm005_mismatched_brackets() {
    let src = "flowchart LR\n    A[Start --> B[End]\n";
    let codes = check(src);
    assert!(
        codes.contains(&"MM005".to_string()),
        "expected MM005, got: {:?}",
        codes
    );
}

#[test]
fn test_sequence_diagram_no_node_checks() {
    // sequenceDiagram doesn't apply flowchart node rules
    let src = "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi\n";
    let codes = check(src);
    assert!(
        !codes.contains(&"MM002".to_string()),
        "MM002 should not fire for sequenceDiagram"
    );
    assert!(
        !codes.contains(&"MM003".to_string()),
        "MM003 should not fire for sequenceDiagram"
    );
}

#[test]
fn test_available_rules() {
    let checker = MermaidChecker::new();
    let rules = checker.available_rules();
    assert!(rules.contains(&"MM001"));
    assert!(rules.contains(&"MM004"));
    assert!(rules.contains(&"MM005"));
}
