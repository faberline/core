use super::*;
use crate::domain::check::lint_config::LintConfig;

fn make_file(source: &str) -> ParsedFile {
    ParsedFile::line_based(source.to_string(), Language::Toml)
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
    diagnostics.iter().map(|d| d.code.as_str()).collect()
}

#[test]
fn test_syntax_error() {
    let source = "[package\nname = \"test\"\n";
    let file = make_file(source);
    let checker = TomlChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"TM001"),
        "expected TM001 for syntax error, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_duplicate_table() {
    let source = "[package]\nname = \"a\"\n[package]\nname = \"b\"\n";
    let file = make_file(source);
    let checker = TomlChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"TM002"),
        "expected TM002 for duplicate table, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_empty_table() {
    let source = "[package]\nname = \"a\"\n[empty]\n[other]\nval = 1\n";
    let file = make_file(source);
    let checker = TomlChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"TM003"),
        "expected TM003 for empty table, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_deprecated_key() {
    let source = "[package]\nauthors-email = \"a@b.com\"\n";
    let file = make_file(source);
    let checker = TomlChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"TM004"),
        "expected TM004 for deprecated key, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_long_string() {
    let long_val = "x".repeat(210);
    let source = format!("[package]\ndescription = \"{}\"\n", long_val);
    let file = make_file(&source);
    let checker = TomlChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        codes(&diags).contains(&"TM005"),
        "expected TM005 for long string, got {:?}",
        codes(&diags)
    );
}

#[test]
fn test_clean_toml() {
    let source = "[package]\nname = \"test\"\nversion = \"0.1.0\"\n";
    let file = make_file(source);
    let checker = TomlChecker::new();
    let diags = checker.check(&file, &LintConfig::default());
    assert!(
        diags.is_empty(),
        "unexpected diagnostics on clean file: {:?}",
        codes(&diags)
    );
}
