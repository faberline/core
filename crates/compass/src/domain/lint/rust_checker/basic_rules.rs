use super::RustChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity};
use crate::domain::syntax::parsed_file::NodeRange;
use crate::syntax::ParsedFile;

impl RustChecker {
    /// Check for unsafe blocks
    pub(super) fn check_unsafe_blocks(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "unsafe_block" && !Self::unsafe_is_documented_or_test(file, node) {
                diagnostics.push(Diagnostic::new(
                    node.to_range(),
                    DiagnosticSeverity::Information,
                    "RS201",
                    DiagnosticCategory::Security,
                    "Unsafe block - ensure memory safety is manually verified",
                ));
            }
            true
        });

        diagnostics
    }

    /// True when an unsafe block has a `// SAFETY:` comment on its line or in the
    /// comment lines above it, or when it is in test code.
    fn unsafe_is_documented_or_test(file: &ParsedFile, node: &tree_sitter::Node<'_>) -> bool {
        let lines: Vec<&str> = file.source.lines().collect();
        let row = node.start_position().row;
        if lines
            .get(row)
            .is_some_and(|line| line.contains("// SAFETY:"))
        {
            return true;
        }

        // Comment lines above the block; a split `let x =` line may sit between them
        let mut c = row.saturating_sub(1);
        if c > 0
            && lines
                .get(c)
                .is_some_and(|line| line.trim_end().ends_with('='))
        {
            c -= 1;
        }
        while let Some(line) = lines.get(c) {
            let text = line.trim_start();
            if !text.starts_with("//") {
                break;
            }
            if text.contains("SAFETY:") {
                return true;
            }
            let Some(previous) = c.checked_sub(1) else {
                break;
            };
            c = previous;
        }

        // Test code: a `mod tests`, or an item with a `#[test]` or `#[cfg(test)]` attribute
        let mut current = node.parent();
        while let Some(ancestor) = current {
            if ancestor.kind() == "mod_item" {
                if let Some(name) = ancestor.child_by_field_name("name") {
                    if file.node_text(&name) == "tests" {
                        return true;
                    }
                }
            }
            if matches!(ancestor.kind(), "mod_item" | "function_item")
                && Self::has_test_attribute(file, &ancestor)
            {
                return true;
            }
            current = ancestor.parent();
        }
        false
    }

    /// True when an item has a `#[test]` or `#[cfg(test)]` attribute directly above it
    fn has_test_attribute(file: &ParsedFile, item: &tree_sitter::Node<'_>) -> bool {
        let mut previous = item.prev_sibling();
        while let Some(sibling) = previous {
            match sibling.kind() {
                "attribute_item" => {
                    let text = file.node_text(&sibling);
                    if text.contains("cfg(test)") || text == "#[test]" {
                        return true;
                    }
                }
                "line_comment" => {}
                _ => break,
            }
            previous = sibling.prev_sibling();
        }
        false
    }

    /// Check for .clone() calls that might be unnecessary
    pub(super) fn check_clone_usage(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if func.kind() == "field_expression" {
                        if let Some(field) = func.child_by_field_name("field") {
                            if file.node_text(&field) == "clone" {
                                diagnostics.push(Diagnostic::new(
                                    node.to_range(),
                                    DiagnosticSeverity::Hint,
                                    "RS001",
                                    DiagnosticCategory::Style,
                                    "Consider if .clone() is necessary - borrowing may be more efficient",
                                ));
                            }
                        }
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// Check for unwrap() calls
    pub(super) fn check_unwrap(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "call_expression" {
                if let Some(func) = node.child_by_field_name("function") {
                    if func.kind() == "field_expression" {
                        if let Some(field) = func.child_by_field_name("field") {
                            let field_name = file.node_text(&field);
                            if field_name == "unwrap" || field_name == "expect" {
                                diagnostics.push(Diagnostic::warning(
                                    node.to_range(),
                                    "RS101",
                                    DiagnosticCategory::Logic,
                                    format!(
                                        ".{}() can panic - consider using ? or match for error handling",
                                        field_name
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// RS102: todo!/unimplemented! macros
    pub(super) fn check_todo_macros(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        file.walk(|node, _depth| {
            if node.kind() == "macro_invocation" {
                if let Some(macro_node) = node.child_by_field_name("macro") {
                    let macro_name = file.node_text(&macro_node);
                    if macro_name == "todo" || macro_name == "unimplemented" {
                        diagnostics.push(Diagnostic::warning(
                            node.to_range(),
                            "RS102",
                            DiagnosticCategory::Logic,
                            format!("{}! macro will panic at runtime", macro_name),
                        ));
                    }
                }
            }
            true
        });

        diagnostics
    }

    /// RS103: dbg! macro left in code
    pub(super) fn check_dbg_macro(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        file.walk(|node, _depth| {
            if node.kind() == "macro_invocation" {
                if let Some(macro_node) = node.child_by_field_name("macro") {
                    if file.node_text(&macro_node) == "dbg" {
                        diagnostics.push(Diagnostic::new(
                            node.to_range(),
                            DiagnosticSeverity::Hint,
                            "RS103",
                            DiagnosticCategory::Style,
                            "dbg! macro should be removed in production",
                        ));
                    }
                }
            }
            true
        });
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::RustChecker;
    use crate::{Checker, Language, LintConfig, MultiParser};

    fn rs201(source: &str) -> usize {
        let mut parser = MultiParser::new().unwrap();
        let file = parser.parse(source, Language::Rust).unwrap();
        RustChecker::new()
            .check(&file, &LintConfig::default())
            .iter()
            .filter(|d| d.code.as_str() == "RS201")
            .count()
    }

    #[test]
    fn rs201_ignores_unsafe_with_safety_comment() {
        assert_eq!(
            rs201("fn f() {\n    // SAFETY: the pointer is valid.\n    unsafe { g() };\n}\n"),
            0
        );
    }

    #[test]
    fn rs201_ignores_safety_comment_above_split_let() {
        assert_eq!(
            rs201("fn f() {\n    // SAFETY: valid.\n    // more text.\n    let m =\n        unsafe { g() };\n}\n"),
            0
        );
    }

    #[test]
    fn rs201_ignores_unsafe_in_test_module() {
        assert_eq!(
            rs201("#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        unsafe { g() };\n    }\n}\n"),
            0
        );
    }

    #[test]
    fn rs201_ignores_unsafe_in_test_function() {
        assert_eq!(rs201("#[test]\nfn t() {\n    unsafe { g() };\n}\n"), 0);
    }

    #[test]
    fn rs201_still_reports_bare_unsafe() {
        assert_eq!(rs201("fn f() {\n    unsafe { g() };\n}\n"), 1);
    }

    #[test]
    fn rs201_still_reports_unsafe_with_unrelated_comment() {
        assert_eq!(
            rs201("fn f() {\n    // call g\n    unsafe { g() };\n}\n"),
            1
        );
    }

    #[test]
    fn rs201_unsafe_on_first_line_does_not_panic() {
        let _ = rs201("unsafe { g() }\n");
    }
}
