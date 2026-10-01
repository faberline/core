use super::DockerfileChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range};

impl DockerfileChecker {
    /// DK009: Hardcoded secrets in ENV/ARG
    pub(super) fn check_hardcoded_secrets(&self, lines: &[&str]) -> Vec<Diagnostic> {
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

        for (line_num, line) in lines.iter().enumerate() {
            if let Some((inst, rest)) = Self::parse_instruction(line) {
                if !inst.eq_ignore_ascii_case("ENV") && !inst.eq_ignore_ascii_case("ARG") {
                    continue;
                }
                let upper_rest = rest.to_uppercase();
                for keyword in SECRET_KEYWORDS {
                    if upper_rest.contains(keyword) {
                        // Check if there's an actual value assigned (not just a declaration)
                        let has_value = rest.contains('=')
                            && rest
                                .split('=')
                                .nth(1)
                                .map(|v| !v.trim().is_empty())
                                .unwrap_or(false);
                        if has_value {
                            diagnostics.push(Diagnostic::new(
                                Range::new(
                                    Position::new(line_num as u32, 0),
                                    Position::new(line_num as u32, line.len() as u32),
                                ),
                                DiagnosticSeverity::Error,
                                "DK009",
                                DiagnosticCategory::Security,
                                format!(
                                    "Hardcoded secret in {} — '{}' should use build args or secrets mount",
                                    inst, keyword,
                                ),
                            ));
                        }
                        break;
                    }
                }
            }
        }

        diagnostics
    }
}
