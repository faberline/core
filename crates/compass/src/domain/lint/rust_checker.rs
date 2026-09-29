use crate::checker::LintConfig;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, Range};
use crate::syntax::{Language, ParsedFile};

mod basic_rules;
mod clippy_rules;

/// Rust checker
pub struct RustChecker;

impl RustChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RustChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl super::checker::Checker for RustChecker {
    fn language(&self) -> Language {
        Language::Rust
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check for syntax errors
        if file.has_errors {
            file.walk(|node, _depth| {
                if node.is_error() || node.is_missing() {
                    diagnostics.push(Diagnostic::error(
                        Range::from_node(node),
                        "RS000",
                        DiagnosticCategory::Syntax,
                        "Syntax error",
                    ));
                }
                true
            });
        }

        diagnostics.extend(self.check_unsafe_blocks(file));
        diagnostics.extend(self.check_clone_usage(file));
        diagnostics.extend(self.check_unwrap(file));
        diagnostics.extend(self.check_todo_macros(file));
        diagnostics.extend(self.check_dbg_macro(file));
        diagnostics.extend(self.check_needless_return(file));
        diagnostics.extend(self.check_large_enum_variant(file));
        diagnostics.extend(self.check_manual_map(file));
        diagnostics.extend(self.check_single_match(file));
        diagnostics.extend(self.check_wildcard_imports(file));
        diagnostics.extend(self.check_missing_docs_public(file));
        diagnostics.extend(self.check_redundant_closure(file));
        diagnostics.extend(self.check_print_macro(file));
        diagnostics.extend(self.check_string_to_string(file));
        diagnostics.extend(self.check_unused_must_use(file));

        diagnostics
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "RS000", // Syntax error
            "RS001", // Unnecessary clone
            "RS006", // needless-return
            "RS007", // large-enum-variant
            "RS008", // manual-map
            "RS009", // single-match
            "RS010", // wildcard-imports
            "RS011", // missing-docs-public
            "RS012", // redundant-closure
            "RS013", // print-macro
            "RS014", // string-to-string
            "RS015", // unused-must-use
            "RS101", // unwrap/expect usage
            "RS102", // todo!/unimplemented!
            "RS103", // dbg! macro
            "RS201", // Unsafe block
        ]
    }
}
