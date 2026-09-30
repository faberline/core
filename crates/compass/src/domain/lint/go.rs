use crate::diagnostic::{Diagnostic, DiagnosticCategory, Range};
use crate::domain::check::lint_config::LintConfig;
use crate::syntax::{Language, ParsedFile};

mod error_rules;
mod function_rules;
mod import_rules;

// ============================================================================
// GoChecker
// ============================================================================

/// Go checker
pub struct GoChecker;

impl GoChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GoChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl super::checker::Checker for GoChecker {
    fn language(&self) -> Language {
        Language::Go
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check for syntax errors from tree-sitter
        if file.has_errors {
            file.walk(|node, _depth| {
                if node.is_error() || node.is_missing() {
                    diagnostics.push(Diagnostic::error(
                        Range::from_node(node),
                        "GO000",
                        DiagnosticCategory::Syntax,
                        "Syntax error",
                    ));
                }
                true
            });
        }

        // Run all checks
        diagnostics.extend(self.check_unchecked_error(file));
        diagnostics.extend(self.check_blank_import(file));
        diagnostics.extend(self.check_shadowed_variable(file));
        diagnostics.extend(self.check_naked_return(file));
        diagnostics.extend(self.check_context_background(file));
        diagnostics.extend(self.check_empty_error_handling(file));
        diagnostics.extend(self.check_sprintf_in_error(file));
        diagnostics.extend(self.check_exported_doc_comment(file));
        diagnostics.extend(self.check_goroutine_leak(file));
        diagnostics.extend(self.check_unused_import(file));

        diagnostics
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "GO000", // Syntax error
            "GO001", // Unchecked error
            "GO002", // Blank import
            "GO003", // Shadowed variable
            "GO004", // Naked return
            "GO005", // context.Background() outside main/init
            "GO006", // Empty error handling
            "GO007", // fmt.Sprintf in error string
            "GO008", // Exported name missing doc comment
            "GO009", // Goroutine without WaitGroup/context
            "GO010", // Unused import
        ]
    }
}
