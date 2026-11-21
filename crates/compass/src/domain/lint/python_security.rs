use crate::diagnostic::{Diagnostic, DiagnosticCategory, Position, Range};
use crate::domain::syntax::parsed_file::NodeRange;
use crate::syntax::ParsedFile;

/// PY301: `eval()` usage
pub(super) fn check_eval_usage(file: &ParsedFile) -> Vec<Diagnostic> {
    check_function_call(
        file,
        "eval",
        "PY301",
        "Use of eval() is a security risk — consider ast.literal_eval() or safer alternatives",
    )
}

/// PY302: `exec()` usage
pub(super) fn check_exec_usage(file: &ParsedFile) -> Vec<Diagnostic> {
    check_function_call(
        file,
        "exec",
        "PY302",
        "Use of exec() is a security risk — avoid executing dynamic code",
    )
}

/// PY303: `pickle.loads()` / `pickle.load()` usage
pub(super) fn check_pickle_usage(file: &ParsedFile) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    file.walk(|node, _depth| {
        if node.kind() == "call" {
            if let Some(func) = node.child_by_field_name("function") {
                let func_text = file.node_text(&func);
                if func_text == "pickle.loads"
                    || func_text == "pickle.load"
                    || func_text == "cPickle.loads"
                    || func_text == "cPickle.load"
                {
                    diagnostics.push(Diagnostic::warning(
                        node.to_range(),
                        "PY303",
                        DiagnosticCategory::Security,
                        format!(
                            "{}() can execute arbitrary code during deserialization — use a safer format",
                            func_text,
                        ),
                    ));
                }
            }
        }
        true
    });

    diagnostics
}

/// PY304: `subprocess` with `shell=True`
pub(super) fn check_subprocess_shell(file: &ParsedFile) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    file.walk(|node, _depth| {
        if node.kind() == "call" {
            if let Some(func) = node.child_by_field_name("function") {
                let func_text = file.node_text(&func);
                let is_subprocess = func_text.starts_with("subprocess.")
                    || func_text == "Popen"
                    || func_text == "call"
                    || func_text == "check_output"
                    || func_text == "check_call";

                if !is_subprocess {
                    return true;
                }

                // Check for shell=True in keyword arguments
                if let Some(args) = node.child_by_field_name("arguments") {
                    let mut cursor = args.walk();
                    for child in args.children(&mut cursor) {
                        if child.kind() == "keyword_argument" {
                            if let Some(name) = child.child_by_field_name("name") {
                                if file.node_text(&name) == "shell" {
                                    if let Some(value) = child.child_by_field_name("value") {
                                        if file.node_text(&value) == "True" {
                                            diagnostics.push(Diagnostic::warning(
                                                node.to_range(),
                                                "PY304",
                                                DiagnosticCategory::Security,
                                                format!(
                                                    "{}() with shell=True is a security risk — use a list of arguments instead",
                                                    func_text,
                                                ),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        true
    });

    diagnostics
}

/// PY305: Hardcoded secrets (strings matching patterns like `password=`, `secret=`, `api_key=`)
pub(super) fn check_hardcoded_secrets(lines: &[&str]) -> Vec<Diagnostic> {
    const SECRET_PATTERNS: &[&str] = &[
        "password=",
        "password =",
        "secret=",
        "secret =",
        "api_key=",
        "api_key =",
        "apikey=",
        "apikey =",
        "token=",
        "token =",
        "private_key=",
        "private_key =",
        "access_key=",
        "access_key =",
        "secret_key=",
        "secret_key =",
    ];

    let mut diagnostics = Vec::new();

    for (line_num, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let lower = trimmed.to_lowercase();
        for pattern in SECRET_PATTERNS {
            if lower.contains(pattern) {
                // Check that this looks like an assignment with a literal value
                // (not just referencing os.environ or a variable)
                let after_pat = &lower[lower.find(pattern).unwrap() + pattern.len()..];
                let after_trimmed = after_pat.trim();
                // Skip if the value references an env var or is empty
                if after_trimmed.starts_with("os.environ")
                    || after_trimmed.starts_with("os.getenv")
                    || after_trimmed.starts_with("environ")
                    || after_trimmed.starts_with("none")
                    || after_trimmed.starts_with("\"\"")
                    || after_trimmed.starts_with("''")
                    || after_trimmed.is_empty()
                {
                    continue;
                }
                // Only flag if it looks like there's a string literal value
                if after_trimmed.starts_with('"')
                    || after_trimmed.starts_with('\'')
                    || after_trimmed.starts_with("b\"")
                    || after_trimmed.starts_with("b'")
                {
                    // A `token` name holds a plain word or a test name, not a secret
                    if pattern.starts_with("token") {
                        let idx = lower.find(pattern).unwrap();
                        let prefix = &lower[..idx];
                        let body = after_trimmed
                            .trim_start_matches('b')
                            .trim_start_matches(['"', '\'']);
                        let literal = body.split(['"', '\'']).next().unwrap_or("");
                        let name_ends_in_token = prefix
                            .chars()
                            .next_back()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_');
                        let plain = !literal
                            .chars()
                            .any(|c| c.is_ascii_digit() || "-+/=".contains(c))
                            && literal.len() <= 24;
                        if literal.starts_with("test_") || (name_ends_in_token && plain) {
                            continue;
                        }
                    }
                    diagnostics.push(Diagnostic::warning(
                        Range::new(
                            Position::new(line_num as u32, 0),
                            Position::new(line_num as u32, line.len() as u32),
                        ),
                        "PY305",
                        DiagnosticCategory::Security,
                        format!(
                            "Possible hardcoded secret matching '{}' — use environment variables or a secrets manager",
                            pattern.trim(),
                        ),
                    ));
                    break; // One diagnostic per line
                }
            }
        }
    }

    diagnostics
}

/// Helper: check for a simple function call by name
fn check_function_call(
    file: &ParsedFile,
    func_name: &str,
    code: &str,
    message: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    file.walk(|node, _depth| {
        if node.kind() == "call" {
            if let Some(func) = node.child_by_field_name("function") {
                let func_text = file.node_text(&func);
                if func_text == func_name {
                    diagnostics.push(Diagnostic::warning(
                        node.to_range(),
                        code,
                        DiagnosticCategory::Security,
                        message,
                    ));
                }
            }
        }
        true
    });

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn py305_ignores_token_name_with_test_name_value() {
        assert_eq!(
            check_hardcoded_secrets(&["    token = \"test_threat_model\""]).len(),
            0
        );
    }

    #[test]
    fn py305_ignores_csrf_token_word_value() {
        assert_eq!(check_hardcoded_secrets(&["csrf_token = \"form\""]).len(), 0);
    }

    #[test]
    fn py305_still_reports_password_literal() {
        assert_eq!(
            check_hardcoded_secrets(&["password = \"hunter2\""]).len(),
            1
        );
    }

    #[test]
    fn py305_still_reports_long_token_literal() {
        assert_eq!(
            check_hardcoded_secrets(&["token = \"abcd-1234-efgh-5678-ijkl\""]).len(),
            1
        );
    }

    #[test]
    fn py305_still_reports_prefixed_token_with_secret_like_value() {
        assert_eq!(
            check_hardcoded_secrets(&["api_token = \"ab12cd34ef56\""]).len(),
            1
        );
    }
}
