use super::{lr, split_statements, SqlChecker};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};

impl SqlChecker {
    /// SQ001: Unmatched parentheses and missing trailing semicolons.
    pub(super) fn check_syntax(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut depth: i32 = 0;
        let mut open_line: Option<u32> = None;
        for (idx, line) in source.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("--") || t.is_empty() {
                continue;
            }
            for ch in t.chars() {
                if ch == '(' {
                    if depth == 0 {
                        open_line = Some(idx as u32);
                    }
                    depth += 1;
                } else if ch == ')' {
                    depth -= 1;
                    if depth < 0 {
                        diags.push(Diagnostic::new(
                            lr(idx as u32),
                            DiagnosticSeverity::Error,
                            "SQ001",
                            DiagnosticCategory::Syntax,
                            "Unmatched closing parenthesis",
                        ));
                        depth = 0;
                    }
                }
            }
        }
        if depth > 0 {
            diags.push(Diagnostic::new(
                lr(open_line.unwrap_or(0)),
                DiagnosticSeverity::Error,
                "SQ001",
                DiagnosticCategory::Syntax,
                format!("Unmatched opening parenthesis ({} unclosed)", depth),
            ));
        }
        let lines_vec: Vec<(usize, &str)> = source.lines().enumerate().collect();
        let last = lines_vec
            .iter()
            .rev()
            .find(|(_, l)| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with("--")
            })
            .map(|&(i, l)| (i, l));
        if let Some((i, l)) = last {
            let t = l.trim();
            if !t.ends_with(';')
                && !t.to_uppercase().starts_with("BEGIN")
                && !t.to_uppercase().starts_with("END")
            {
                diags.push(Diagnostic::new(
                    lr(i as u32),
                    DiagnosticSeverity::Warning,
                    "SQ001",
                    DiagnosticCategory::Syntax,
                    "Statement does not end with a semicolon",
                ));
            }
        }
        diags
    }

    /// SQ002: SELECT * usage
    pub(super) fn check_select_star(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let u = line.to_uppercase();
            let t = u.trim();
            if t.starts_with("--") {
                continue;
            }
            if t.contains("SELECT *") || t.contains("SELECT  *") {
                diags.push(Diagnostic::new(
                    lr(i as u32),
                    DiagnosticSeverity::Warning,
                    "SQ002",
                    DiagnosticCategory::Style,
                    "Avoid SELECT * — specify column names explicitly",
                ));
            }
        }
        diags
    }

    /// SQ003: Missing WHERE on UPDATE/DELETE.
    pub(super) fn check_missing_where(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for stmt in &split_statements(source) {
            let u = stmt.text.to_uppercase();
            let t = u.trim();
            if (t.starts_with("UPDATE ") || t.starts_with("DELETE ")) && !u.contains("WHERE") {
                let kind = if t.starts_with("UPDATE") {
                    "UPDATE"
                } else {
                    "DELETE"
                };
                diags.push(Diagnostic::new(
                    lr(stmt.start_line),
                    DiagnosticSeverity::Warning,
                    "SQ003",
                    DiagnosticCategory::Logic,
                    format!("{} without WHERE clause — this affects all rows", kind),
                ));
            }
        }
        diags
    }

    /// SQ004: Implicit join (FROM table1, table2).
    pub(super) fn check_implicit_join(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let u = line.to_uppercase();
            let t = u.trim();
            if t.starts_with("--") {
                continue;
            }
            if let Some(fp) = t.find("FROM ") {
                let after = &t[fp + 5..];
                let bw = after.find("WHERE").map_or(after, |w| &after[..w]);
                if !bw.contains("JOIN") && bw.contains(',') {
                    diags.push(Diagnostic::new(
                        lr(i as u32),
                        DiagnosticSeverity::Warning,
                        "SQ004",
                        DiagnosticCategory::Style,
                        "Implicit join — use explicit JOIN syntax",
                    ));
                }
            }
        }
        diags
    }

    /// SQ005: Deprecated CONVERT -> CAST.
    pub(super) fn check_deprecated_functions(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let t = line.to_uppercase();
            if t.trim().starts_with("--") {
                continue;
            }
            if t.contains("CONVERT(") {
                diags.push(Diagnostic::new(
                    lr(i as u32),
                    DiagnosticSeverity::Hint,
                    "SQ005",
                    DiagnosticCategory::Style,
                    "CONVERT() deprecated — prefer CAST(expr AS type)",
                ));
            }
        }
        diags
    }

    /// PG001: SERIAL -> GENERATED ALWAYS AS IDENTITY.
    pub(super) fn check_serial_type(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let u = line.to_uppercase();
            let t = u.trim();
            if t.starts_with("--") {
                continue;
            }
            if t.contains("SERIAL") && !t.contains("GENERATED") && !t.contains("SERIALIZABLE") {
                diags.push(Diagnostic::new(
                    lr(i as u32),
                    DiagnosticSeverity::Hint,
                    "PG001",
                    DiagnosticCategory::Style,
                    "Consider GENERATED ALWAYS AS IDENTITY instead of SERIAL",
                ));
            }
        }
        diags
    }

    /// MY001: Missing ENGINE= on CREATE TABLE.
    pub(super) fn check_missing_engine(&self, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for stmt in &split_statements(source) {
            let u = stmt.text.to_uppercase();
            if u.trim().starts_with("CREATE TABLE") && !u.contains("ENGINE") {
                diags.push(Diagnostic::new(
                    lr(stmt.start_line),
                    DiagnosticSeverity::Hint,
                    "MY001",
                    DiagnosticCategory::Style,
                    "CREATE TABLE without ENGINE= — consider specifying ENGINE=InnoDB",
                ));
            }
        }
        diags
    }
}
