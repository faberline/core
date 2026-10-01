use super::checker::Checker;
use crate::checker::LintConfig;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range};
use crate::syntax::{Language, ParsedFile};

mod ast_rules;
mod line_rules;

pub struct SqlChecker;

impl SqlChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SqlChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect SQL injection patterns in Python/JS/Go source code.
pub fn detect_sql_injection(source: &str, language: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let kws = ["SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE"];
    for (i, line) in source.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('#') || t.starts_with("//") {
            continue;
        }
        let u = t.to_uppercase();
        if !kws.iter().any(|kw| u.contains(kw)) {
            continue;
        }
        let ln = i as u32;
        match language {
            "python" | "py" => {
                if t.contains("f\"") || t.contains("f'") {
                    diags.push(inj(ln, "SQL in f-string — use parameterized queries"));
                }
                if t.contains("\" %") || t.contains("' %") {
                    diags.push(inj(ln, "SQL with % formatting — use parameterized queries"));
                }
                if t.contains(".format(") {
                    diags.push(inj(ln, "SQL with .format() — use parameterized queries"));
                }
            }
            "javascript" | "js" | "typescript" | "ts" => {
                if t.contains('`') && t.contains("${") {
                    diags.push(inj(
                        ln,
                        "SQL in template literal — use parameterized queries",
                    ));
                }
                if (t.contains("\"SELECT") || t.contains("'SELECT")) && t.contains(" + ") {
                    diags.push(inj(
                        ln,
                        "SQL string concatenation — use parameterized queries",
                    ));
                }
            }
            "go" => {
                if t.contains("fmt.Sprintf") || t.contains("fmt.Fprintf") {
                    diags.push(inj(ln, "SQL with fmt.Sprintf — use parameterized queries"));
                }
                if (t.contains("\"SELECT") || t.contains("\"INSERT")) && t.contains(" + ") {
                    diags.push(inj(
                        ln,
                        "SQL string concatenation — use parameterized queries",
                    ));
                }
            }
            _ => {}
        }
    }
    diags
}

fn inj(line: u32, msg: &str) -> Diagnostic {
    Diagnostic::new(
        lr(line),
        DiagnosticSeverity::Warning,
        "SQL-INJ",
        DiagnosticCategory::Security,
        msg,
    )
}

struct StatementSpan {
    text: String,
    start_line: u32,
}

fn split_statements(source: &str) -> Vec<StatementSpan> {
    let mut stmts = Vec::new();
    let mut cur = String::new();
    let mut start: u32 = 0;
    let mut found = false;
    for (i, line) in source.lines().enumerate() {
        let t = line.trim();
        if t.starts_with("--") || t.is_empty() {
            continue;
        }
        if !found {
            start = i as u32;
            found = true;
        }
        cur.push(' ');
        cur.push_str(t);
        if t.ends_with(';') {
            stmts.push(StatementSpan {
                text: cur.clone(),
                start_line: start,
            });
            cur.clear();
            found = false;
        }
    }
    if !cur.trim().is_empty() {
        stmts.push(StatementSpan {
            text: cur,
            start_line: start,
        });
    }
    stmts
}

fn lr(line: u32) -> Range {
    Range::new(Position::new(line, 0), Position::new(line, u32::MAX))
}

impl Checker for SqlChecker {
    fn language(&self) -> Language {
        Language::Sql
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut d = Vec::new();

        // R3: When a real tree-sitter SQL tree is available, derive syntax errors
        // directly from AST parse errors and use AST node walking for semantic checks.
        if file.language == Language::Sql && !file.has_errors && !file.is_line_based {
            // SQ001 via AST: tree-sitter syntax errors
            d.extend(self.ast_check_syntax(file));
            // Semantic checks use AST node matching (SELECT *, missing WHERE, etc.)
            d.extend(self.ast_check_select_star(file));
            d.extend(self.ast_check_missing_where(file));
            d.extend(self.ast_check_implicit_join(file));
            // Rule-based checks still line-based as they match on keyword patterns
            d.extend(self.check_deprecated_functions(&file.source));
            d.extend(self.check_serial_type(&file.source));
            d.extend(self.check_missing_engine(&file.source));
        } else {
            // Line-based fallback (dummy tree / has_errors)
            d.extend(self.check_syntax(&file.source));
            d.extend(self.check_select_star(&file.source));
            d.extend(self.check_missing_where(&file.source));
            d.extend(self.check_implicit_join(&file.source));
            d.extend(self.check_deprecated_functions(&file.source));
            d.extend(self.check_serial_type(&file.source));
            d.extend(self.check_missing_engine(&file.source));
        }

        d
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "SQ001", "SQ002", "SQ003", "SQ004", "SQ005", "PG001", "MY001",
        ]
    }
}

#[cfg(test)]
mod tests;
