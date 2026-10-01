use super::{lr, SqlChecker};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range};
use crate::syntax::ParsedFile;

impl SqlChecker {
    // =========================================================================
    // AST-based checks (tree-sitter-sql node kinds) — R3
    // =========================================================================

    /// SQ001 via AST: map tree-sitter parse errors to diagnostics.
    pub(super) fn ast_check_syntax(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        file.collect_errors()
            .into_iter()
            .map(|err| {
                let (row, col) = err.start_position;
                let pos = Position::new(
                    (row.saturating_sub(1)) as u32,
                    (col.saturating_sub(1)) as u32,
                );
                Diagnostic::new(
                    Range::new(pos, pos),
                    DiagnosticSeverity::Error,
                    "SQ001",
                    DiagnosticCategory::Syntax,
                    "SQL syntax error",
                )
            })
            .collect()
    }

    /// SQ002 via AST: detect SELECT * via wildcard nodes in select clauses.
    pub(super) fn ast_check_select_star(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        file.walk(|node, _depth| {
            if matches!(node.kind(), "wildcard" | "star" | "asterisk" | "all_fields") {
                let in_select = node
                    .parent()
                    .map(|p| {
                        matches!(
                            p.kind(),
                            "select_clause" | "select_expression" | "select_item"
                        )
                    })
                    .unwrap_or(false);
                if in_select {
                    let line = node.start_position().row as u32;
                    diags.push(Diagnostic::new(
                        lr(line),
                        DiagnosticSeverity::Warning,
                        "SQ002",
                        DiagnosticCategory::Style,
                        "Avoid SELECT * — list explicit columns for clarity and safety",
                    ));
                }
            }
            true
        });
        diags
    }

    /// SQ003 via AST: DELETE/UPDATE statements without a WHERE clause.
    pub(super) fn ast_check_missing_where(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        file.walk(|node, _depth| {
            if matches!(
                node.kind(),
                "delete_statement" | "update_statement" | "delete" | "update"
            ) {
                let has_where = (0..node.child_count())
                    .filter_map(|i| node.child(i))
                    .any(|c| matches!(c.kind(), "where_clause" | "where"));
                if !has_where {
                    let line = node.start_position().row as u32;
                    let stmt = if node.kind().starts_with("delete") {
                        "DELETE"
                    } else {
                        "UPDATE"
                    };
                    diags.push(Diagnostic::new(
                        lr(line),
                        DiagnosticSeverity::Warning,
                        "SQ003",
                        DiagnosticCategory::Logic,
                        format!("{} without WHERE clause — will affect all rows", stmt),
                    ));
                }
            }
            true
        });
        diags
    }

    /// SQ004 via AST: implicit JOIN via comma-separated tables in FROM clause.
    pub(super) fn ast_check_implicit_join(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        file.walk(|node, _depth| {
            if matches!(node.kind(), "from_clause" | "from") {
                let mut cursor = node.walk();
                let comma_count = node
                    .children(&mut cursor)
                    .filter(|c| c.kind() == ",")
                    .count();
                if comma_count > 0 {
                    let line = node.start_position().row as u32;
                    diags.push(Diagnostic::new(
                        lr(line),
                        DiagnosticSeverity::Warning,
                        "SQ004",
                        DiagnosticCategory::Style,
                        "Implicit JOIN (comma-separated tables) — use explicit JOIN syntax",
                    ));
                }
            }
            true
        });
        diags
    }
}
