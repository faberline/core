use super::*;
use crate::checker::LintConfig;

fn make(s: &str) -> ParsedFile {
    ParsedFile::line_based(s.to_string(), Language::GraphQL)
}
fn codes(d: &[Diagnostic]) -> Vec<&str> {
    d.iter().map(|d| d.code.as_str()).collect()
}

#[test]
fn test_unmatched_braces() {
    let d = GraphqlChecker::new().check(
        &make("type User {\n  id: ID!\n  name: String\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"GQ001"),
        "expected GQ001, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_deprecated_field() {
    let d = GraphqlChecker::new().check(
        &make("type User {\n  id: ID!\n  old: String @deprecated(reason: \"Use name\")\n}\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"GQ003"),
        "expected GQ003, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_deep_nesting() {
    let src = "query {\n  a {\n    b {\n      c {\n        d {\n          e {\n            f\n          }\n        }\n      }\n    }\n  }\n}\n";
    let d = GraphqlChecker::new().check(&make(src), &LintConfig::default());
    assert!(
        codes(&d).contains(&"GQ004"),
        "expected GQ004, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_unused_fragment() {
    let src =
        "fragment UserFields on User {\n  id\n  name\n}\nquery {\n  users {\n    id\n  }\n}\n";
    let d = GraphqlChecker::new().check(&make(src), &LintConfig::default());
    assert!(
        codes(&d).contains(&"GQ006"),
        "expected GQ006, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_duplicate_field() {
    let d = GraphqlChecker::new().check(
        &make("query {\n  users {\n    id\n    name\n    id\n  }\n}\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"GQ007"),
        "expected GQ007, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_missing_description() {
    let d =
        GraphqlChecker::new().check(&make("type User {\n  id: ID!\n}\n"), &LintConfig::default());
    assert!(
        codes(&d).contains(&"GQ005"),
        "expected GQ005, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_fragment_used() {
    let src = "fragment UserFields on User {\n  id\n  name\n}\nquery {\n  users {\n    ...UserFields\n  }\n}\n";
    let d = GraphqlChecker::new().check(&make(src), &LintConfig::default());
    assert!(
        !codes(&d).contains(&"GQ006"),
        "should not flag used fragment, got {:?}",
        codes(&d)
    );
}
