use crate::diagnostic::{Diagnostic, DiagnosticCategory, Range};
use crate::domain::check::lint_config::LintConfig;
use crate::syntax::{Language, ParsedFile};

mod basic_rules;
mod eslint_rules;

/// TypeScript checker
pub struct TypeScriptChecker;

impl TypeScriptChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TypeScriptChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl super::checker::Checker for TypeScriptChecker {
    fn language(&self) -> Language {
        Language::TypeScript
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check for syntax errors
        if file.has_errors {
            file.walk(|node, _depth| {
                if node.is_error() || node.is_missing() {
                    diagnostics.push(Diagnostic::error(
                        Range::from_node(node),
                        "TS000",
                        DiagnosticCategory::Syntax,
                        "Syntax error",
                    ));
                }
                true
            });
        }

        diagnostics.extend(self.check_any_type(file));
        diagnostics.extend(self.check_non_null_assertion(file));
        diagnostics.extend(self.check_type_assertion(file));
        diagnostics.extend(self.check_console_log(file));
        diagnostics.extend(self.check_prefer_const(file));
        diagnostics.extend(self.check_floating_promises(file));
        diagnostics.extend(self.check_strict_boolean(file));
        diagnostics.extend(self.check_unnecessary_type_assertion(file));
        diagnostics.extend(self.check_empty_interface(file));
        diagnostics.extend(self.check_duplicate_enum_values(file));
        diagnostics.extend(self.check_prefer_optional_chain(file));
        diagnostics.extend(self.check_no_namespace(file));
        diagnostics.extend(self.check_explicit_return_type(file));
        diagnostics.extend(self.check_no_var_requires(file));
        diagnostics.extend(self.check_consistent_type_imports(file));

        diagnostics
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "TS000", // Syntax error
            "TS001", // any type
            "TS002", // Type assertion
            "TS006", // no-floating-promises
            "TS007", // strict-boolean-expressions
            "TS008", // no-unnecessary-type-assertion
            "TS009", // no-empty-interface
            "TS010", // no-duplicate-enum-values
            "TS011", // prefer-optional-chain
            "TS012", // no-namespace
            "TS013", // explicit-function-return-type
            "TS014", // no-var-requires
            "TS015", // consistent-type-imports
            "TS102", // Non-null assertion
            "TS103", // console.log
            "TS104", // Prefer const
        ]
    }
}
