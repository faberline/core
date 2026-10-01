use super::*;
use crate::domain::check::lint_config::LintConfig;

fn make(s: &str) -> ParsedFile {
    ParsedFile::line_based(s.to_string(), Language::Proto)
}
fn codes(d: &[Diagnostic]) -> Vec<&str> {
    d.iter().map(|d| d.code.as_str()).collect()
}

#[test]
fn test_missing_package() {
    let d = ProtoChecker::new().check(
        &make("syntax = \"proto3\";\nmessage Foo {\n  string name = 1;\n}\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"PB004"),
        "expected PB004, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_duplicate_field_number() {
    let d = ProtoChecker::new().check(
        &make("syntax = \"proto3\";\npackage test;\nmessage Foo {\n  string a = 1;\n  string b = 1;\n}\n"),
        &LintConfig::default());
    assert!(
        codes(&d).contains(&"PB002"),
        "expected PB002, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_empty_service() {
    let d = ProtoChecker::new().check(
        &make("syntax = \"proto3\";\npackage test;\nservice MyService {\n}\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"PB005"),
        "expected PB005, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_field_naming() {
    let d = ProtoChecker::new().check(
        &make("syntax = \"proto3\";\npackage test;\nmessage Foo {\n  string myField = 1;\n}\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"PB007"),
        "expected PB007, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_clean_proto() {
    let src = "syntax = \"proto3\";\npackage test;\nmessage Foo {\n  string my_field = 1;\n  int32 count = 2;\n}\nservice FooService {\n  rpc GetFoo (Foo) returns (Foo);\n}\n";
    let d = ProtoChecker::new().check(&make(src), &LintConfig::default());
    assert!(d.is_empty(), "unexpected diagnostics: {:?}", codes(&d));
}

#[test]
fn test_snake_case_helper() {
    assert!(is_snake_case("my_field"));
    assert!(is_snake_case("name"));
    assert!(!is_snake_case("myField"));
    assert!(!is_snake_case("MyField"));
}
