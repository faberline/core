use super::*;
use crate::domain::check::lint_config::LintConfig;

fn make(s: &str) -> ParsedFile {
    ParsedFile::line_based(s.to_string(), Language::Sql)
}
fn codes(d: &[Diagnostic]) -> Vec<&str> {
    d.iter().map(|d| d.code.as_str()).collect()
}

#[test]
fn test_unmatched_parens() {
    let d = SqlChecker::new().check(
        &make("SELECT (id, name FROM users;\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"SQ001"),
        "expected SQ001, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_select_star() {
    let d = SqlChecker::new().check(&make("SELECT * FROM users;\n"), &LintConfig::default());
    assert!(
        codes(&d).contains(&"SQ002"),
        "expected SQ002, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_missing_where_delete() {
    let d = SqlChecker::new().check(&make("DELETE FROM users;\n"), &LintConfig::default());
    assert!(
        codes(&d).contains(&"SQ003"),
        "expected SQ003, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_implicit_join() {
    let d = SqlChecker::new().check(
        &make("SELECT a.id FROM users a, orders b WHERE a.id = b.user_id;\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"SQ004"),
        "expected SQ004, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_deprecated_convert() {
    let d = SqlChecker::new().check(
        &make("SELECT CONVERT(varchar, d) FROM t;\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"SQ005"),
        "expected SQ005, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_serial_type() {
    let d = SqlChecker::new().check(
        &make("CREATE TABLE t (\n  id SERIAL PRIMARY KEY\n);\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"PG001"),
        "expected PG001, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_missing_engine() {
    let d = SqlChecker::new().check(
        &make("CREATE TABLE users (\n  id INT PRIMARY KEY\n);\n"),
        &LintConfig::default(),
    );
    assert!(
        codes(&d).contains(&"MY001"),
        "expected MY001, got {:?}",
        codes(&d)
    );
}

#[test]
fn test_injection_python() {
    let d = detect_sql_injection("q = f\"SELECT * FROM users WHERE id = {uid}\"\n", "python");
    assert!(d.iter().any(|d| d.code == "SQL-INJ"), "expected SQL-INJ");
}

#[test]
fn test_injection_js() {
    let d = detect_sql_injection(
        "const q = `SELECT * FROM users WHERE id = ${uid}`;\n",
        "javascript",
    );
    assert!(d.iter().any(|d| d.code == "SQL-INJ"), "expected SQL-INJ");
}

#[test]
fn test_injection_go() {
    let d = detect_sql_injection(
        "q := fmt.Sprintf(\"SELECT * FROM u WHERE id = %s\", id)\n",
        "go",
    );
    assert!(d.iter().any(|d| d.code == "SQL-INJ"), "expected SQL-INJ");
}

#[test]
fn injection_ignores_wait_message_with_delete_word() {
    let d = detect_sql_injection(
        "print(f\"waiting for delete of {name} to finish\")\n",
        "python",
    );
    assert!(d.is_empty(), "expected no finding, got {:?}", codes(&d));
}

#[test]
fn injection_ignores_log_line_with_update_word() {
    let d = detect_sql_injection("log.info(f\"update {n} done\")\n", "python");
    assert!(d.is_empty(), "expected no finding, got {:?}", codes(&d));
}

#[test]
fn injection_ignores_js_template_with_create_word() {
    let d = detect_sql_injection("const m = `create ${x} now`;\n", "javascript");
    assert!(d.is_empty(), "expected no finding, got {:?}", codes(&d));
}

#[test]
fn injection_ignores_go_sprintf_with_drop_word() {
    let d = detect_sql_injection("m := fmt.Sprintf(\"drop %s\", x)\n", "go");
    assert!(d.is_empty(), "expected no finding, got {:?}", codes(&d));
}

#[test]
fn injection_still_reports_python_percent_format() {
    let d = detect_sql_injection(
        "q = \"SELECT * FROM users WHERE id = %s\" % uid\n",
        "python",
    );
    assert!(d.iter().any(|d| d.code == "SQL-INJ"), "expected SQL-INJ");
}

#[test]
fn injection_still_reports_python_format_call() {
    let d = detect_sql_injection(
        "q = \"DELETE FROM users WHERE id = {}\".format(uid)\n",
        "python",
    );
    assert!(d.iter().any(|d| d.code == "SQL-INJ"), "expected SQL-INJ");
}
