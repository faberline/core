use super::PythonChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory};
use crate::domain::syntax::parsed_file::NodeRange;
use crate::syntax::ParsedFile;

impl PythonChecker {
    // ===== Complexity Checks (pylint-like) =====

    /// PY701: Too many arguments, PY702: Function too long
    pub(super) fn check_function_complexity(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        const MAX_ARGS: usize = 7;
        const MAX_LINES: usize = 50;

        file.walk(|node, _depth| {
            if node.kind() == "function_definition" || node.kind() == "async_function_definition" {
                let name = node
                    .child_by_field_name("name")
                    .map(|n| file.node_text(&n).to_string())
                    .unwrap_or_default();

                // Check argument count
                if let Some(params) = node.child_by_field_name("parameters") {
                    let mut count = 0;
                    let mut cursor = params.walk();
                    for child in params.children(&mut cursor) {
                        if matches!(
                            child.kind(),
                            "identifier"
                                | "typed_parameter"
                                | "default_parameter"
                                | "typed_default_parameter"
                                | "list_splat_pattern"
                                | "dictionary_splat_pattern"
                        ) {
                            count += 1;
                        }
                    }
                    if count > MAX_ARGS {
                        diagnostics.push(Diagnostic::new(
                            params.to_range(),
                            crate::diagnostic::DiagnosticSeverity::Hint,
                            "PY701",
                            DiagnosticCategory::Style,
                            format!(
                                "Function '{}' has {} arguments (max {})",
                                name, count, MAX_ARGS
                            ),
                        ));
                    }
                }

                // Check function length
                let start = node.start_position().row;
                let end = node.end_position().row;
                let lines = end - start + 1;
                if lines > MAX_LINES {
                    diagnostics.push(Diagnostic::new(
                        node.to_range(),
                        crate::diagnostic::DiagnosticSeverity::Hint,
                        "PY702",
                        DiagnosticCategory::Style,
                        format!("Function '{}' is {} lines (max {})", name, lines, MAX_LINES),
                    ));
                }
            }
            true
        });

        diagnostics
    }
}
