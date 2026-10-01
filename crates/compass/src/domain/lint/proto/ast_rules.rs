use super::{lr, ProtoChecker};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range};
use crate::syntax::ParsedFile;

impl ProtoChecker {
    // =========================================================================
    // AST-based checks (tree-sitter-proto node kinds) — R3
    // =========================================================================

    /// PB001 via AST: map tree-sitter parse errors to diagnostics.
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
                    "PB001",
                    DiagnosticCategory::Syntax,
                    "Protobuf syntax error",
                )
            })
            .collect()
    }

    /// PB002 via AST: detect duplicate field numbers within the same message.
    pub(super) fn ast_check_duplicate_field_numbers(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let source = file.source.as_bytes();

        file.walk(|node, _depth| {
            // message_body or message_block contains field definitions
            if matches!(node.kind(), "message_body" | "message_block" | "message") {
                let mut seen: std::collections::HashMap<String, u32> =
                    std::collections::HashMap::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if matches!(child.kind(), "field" | "message_field") {
                        // Field number is typically in a child named "number" or "field_number"
                        if let Some(num_node) = child
                            .child_by_field_name("number")
                            .or_else(|| child.child_by_field_name("field_number"))
                        {
                            if let Ok(num) = num_node.utf8_text(source) {
                                let line = child.start_position().row as u32;
                                if let Some(&prev) = seen.get(num) {
                                    diags.push(Diagnostic::new(
                                        lr(line),
                                        DiagnosticSeverity::Error,
                                        "PB002",
                                        DiagnosticCategory::Logic,
                                        format!(
                                            "Duplicate field number {} (first at line {})",
                                            num,
                                            prev + 1
                                        ),
                                    ));
                                } else {
                                    seen.insert(num.to_string(), line);
                                }
                            }
                        }
                    }
                }
            }
            true
        });
        diags
    }

    /// PB004 via AST: missing `package` declaration at top level.
    pub(super) fn ast_check_missing_package(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut has_package = false;
        file.walk(|node, _depth| {
            if node.kind() == "package" || node.kind() == "package_statement" {
                has_package = true;
            }
            true
        });
        if !has_package {
            vec![Diagnostic::new(
                lr(0),
                DiagnosticSeverity::Warning,
                "PB004",
                DiagnosticCategory::Style,
                "Missing package declaration",
            )]
        } else {
            vec![]
        }
    }

    /// PB005 via AST: service definitions with no RPC methods.
    pub(super) fn ast_check_empty_service(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let source = file.source.as_bytes();

        file.walk(|node, _depth| {
            if matches!(node.kind(), "service" | "service_definition") {
                let has_rpc = (0..node.child_count())
                    .filter_map(|i| node.child(i))
                    .any(|c| matches!(c.kind(), "rpc" | "rpc_definition" | "rpc_statement"));
                if !has_rpc {
                    let name = node
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source).ok())
                        .unwrap_or("unknown");
                    let line = node.start_position().row as u32;
                    diags.push(Diagnostic::new(
                        lr(line),
                        DiagnosticSeverity::Warning,
                        "PB005",
                        DiagnosticCategory::Logic,
                        format!("Service '{}' has no RPC methods", name),
                    ));
                }
            }
            true
        });
        diags
    }
}
