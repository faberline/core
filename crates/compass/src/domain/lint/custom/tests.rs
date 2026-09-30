use super::*;
use crate::syntax::Language;

fn line_file(src: &str) -> ParsedFile {
    ParsedFile::line_based(src.to_string(), Language::Markdown)
}

fn load_engine(toml: &str) -> CustomLintEngine {
    let f: CustomRulesFile = ::toml::from_str(toml).expect("valid toml");
    CustomLintEngine::from_rules_file(&f)
}

// -----------------------------------------------------------------------

#[test]
fn test_regex_rule_matches_todo() {
    let engine = load_engine(
        r#"
[[rule]]
id       = "NO_TODO"
kind     = "regex"
pattern  = "TODO"
severity = "warning"
message  = "TODO comment found"
"#,
    );
    let file = line_file("// TODO: fix this\nfn main() {}");
    let diags = engine.check(&file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "CUSTOM_NO_TODO");
    assert_eq!(diags[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(diags[0].category, DiagnosticCategory::Custom);
}

#[test]
fn test_no_match_returns_empty() {
    let engine = load_engine(
        r#"
[[rule]]
id      = "NO_HACK"
kind    = "regex"
pattern = "HACK"
message = "HACK comment"
"#,
    );
    let file = line_file("fn clean_code() {}");
    assert!(engine.check(&file).is_empty());
}

#[test]
fn test_custom_prefix() {
    let cfg = CustomRuleConfig {
        id: "MY_RULE".to_string(),
        kind: RuleKind::Regex,
        pattern: "x".to_string(),
        severity: "error".to_string(),
        message: "test".to_string(),
        fix: None,
    };
    assert_eq!(cfg.code(), "CUSTOM_MY_RULE");
}

#[test]
fn test_rule_count() {
    let engine = load_engine(
        r#"
[[rule]]
id = "A"
kind = "regex"
pattern = "foo"
message = "a"

[[rule]]
id = "B"
kind = "regex"
pattern = "bar"
message = "b"
"#,
    );
    assert_eq!(engine.rule_count(), 2);
}

#[test]
fn test_invalid_regex_skipped() {
    let engine = load_engine(
        r#"
[[rule]]
id      = "BAD"
kind    = "regex"
pattern = "[unclosed"
message = "bad rule"
"#,
    );
    // Invalid regex must be silently skipped — engine stays functional.
    assert_eq!(engine.rule_count(), 0);
}

#[test]
fn test_fix_hint_attached() {
    let engine = load_engine(
        r#"
[[rule]]
id      = "NO_PRINT"
kind    = "regex"
pattern = "println!"
message = "No println! in production"
fix     = "Replace println! with tracing::info!"
"#,
    );
    let file = line_file("println!(\"hello\");");
    let diags = engine.check(&file);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].quick_fixes.len(), 1);
    assert_eq!(
        diags[0].quick_fixes[0].title,
        "Replace println! with tracing::info!"
    );
}

#[test]
fn test_severity_parsing() {
    let cfgs = [
        ("error", DiagnosticSeverity::Error),
        ("warning", DiagnosticSeverity::Warning),
        ("information", DiagnosticSeverity::Information),
        ("hint", DiagnosticSeverity::Hint),
        ("WARN", DiagnosticSeverity::Warning),
        ("unknown", DiagnosticSeverity::Warning),
    ];
    for (sev_str, expected) in &cfgs {
        let cfg = CustomRuleConfig {
            id: "X".to_string(),
            kind: RuleKind::Regex,
            pattern: "x".to_string(),
            severity: sev_str.to_string(),
            message: "m".to_string(),
            fix: None,
        };
        assert_eq!(
            cfg.diagnostic_severity(),
            *expected,
            "failed for '{}'",
            sev_str
        );
    }
}

#[test]
fn test_multiple_matches_on_same_file() {
    let engine = load_engine(
        r#"
[[rule]]
id      = "NO_TODO"
kind    = "regex"
pattern = "TODO"
message = "TODO found"
"#,
    );
    let file = line_file("// TODO: first\n// normal\n// TODO: second\n");
    let diags = engine.check(&file);
    assert_eq!(diags.len(), 2);
}

#[test]
fn test_rule_codes() {
    let engine = load_engine(
        r#"
[[rule]]
id = "ALPHA"
kind = "regex"
pattern = "alpha"
message = "a"
"#,
    );
    let codes = engine.rule_codes();
    assert!(codes.contains(&RuleCode::from("CUSTOM_ALPHA")));
}
