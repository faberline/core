use super::GitlabCiChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range};

impl GitlabCiChecker {
    /// GL008: Hardcoded secrets in variables
    pub(super) fn check_hardcoded_secrets(lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        const SECRET_KEYWORDS: &[&str] = &[
            "PASSWORD",
            "SECRET",
            "TOKEN",
            "API_KEY",
            "APIKEY",
            "PRIVATE_KEY",
            "ACCESS_KEY",
            "CREDENTIAL",
        ];

        let mut in_variables = false;
        let mut variables_indent: Option<usize> = None;

        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let indent = line.len() - line.trim_start().len();

            if trimmed == "variables:" {
                in_variables = true;
                variables_indent = Some(indent);
                continue;
            }

            if in_variables {
                if let Some(vi) = variables_indent {
                    if indent <= vi && !trimmed.is_empty() {
                        in_variables = false;
                        variables_indent = None;
                    } else {
                        let upper = trimmed.to_uppercase();
                        for keyword in SECRET_KEYWORDS {
                            if upper.contains(keyword) {
                                if let Some(val) = Self::extract_yaml_value(trimmed) {
                                    let val_trimmed = val.trim();
                                    if !val_trimmed.is_empty()
                                        && !val_trimmed.starts_with('$')
                                        && !val_trimmed.starts_with("${")
                                    {
                                        diagnostics.push(Diagnostic::new(
                                            Range::new(
                                                Position::new(line_num as u32, 0),
                                                Position::new(line_num as u32, line.len() as u32),
                                            ),
                                            DiagnosticSeverity::Error,
                                            "GL008",
                                            DiagnosticCategory::Security,
                                            format!(
                                                "Hardcoded secret in CI variable — '{}' should use CI/CD masked variables",
                                                keyword,
                                            ),
                                        ));
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        diagnostics
    }
}
