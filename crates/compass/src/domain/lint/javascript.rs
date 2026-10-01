use crate::checker::LintConfig;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, Range};
use crate::syntax::{Language, ParsedFile};

mod basic_rules;
mod eslint_rules;

/// JavaScript checker
pub struct JavaScriptChecker;

impl JavaScriptChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JavaScriptChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl super::checker::Checker for JavaScriptChecker {
    fn language(&self) -> Language {
        Language::JavaScript
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check for syntax errors from tree-sitter
        if file.has_errors {
            file.walk(|node, _depth| {
                if node.is_error() || node.is_missing() {
                    diagnostics.push(Diagnostic::error(
                        Range::from_node(node),
                        "JS000",
                        DiagnosticCategory::Syntax,
                        "Syntax error",
                    ));
                }
                true
            });
        }

        // Run all checks
        diagnostics.extend(self.check_var_usage(file));
        diagnostics.extend(self.check_console_log(file));
        diagnostics.extend(self.check_loose_equality(file));
        diagnostics.extend(self.check_eval_usage(file));
        diagnostics.extend(self.check_debugger_statement(file));
        diagnostics.extend(self.check_implied_eval(file));
        diagnostics.extend(self.check_no_proto(file));
        diagnostics.extend(self.check_no_with(file));
        diagnostics.extend(self.check_no_alert(file));
        diagnostics.extend(self.check_no_caller(file));
        diagnostics.extend(self.check_no_extend_native(file));
        diagnostics.extend(self.check_no_new_wrappers(file));
        diagnostics.extend(self.check_no_throw_literal(file));
        diagnostics.extend(self.check_no_return_assign(file));
        diagnostics.extend(self.check_no_self_compare(file));

        diagnostics
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "JS000", // Syntax error
            "JS001", // var usage
            "JS002", // console.log
            "JS003", // Loose equality (== / !=)
            "JS004", // eval() usage
            "JS005", // debugger statement
            "JS006", // no-implied-eval
            "JS007", // no-proto
            "JS008", // no-with
            "JS009", // no-alert
            "JS010", // no-caller
            "JS011", // no-extend-native
            "JS012", // no-new-wrappers
            "JS013", // no-throw-literal
            "JS014", // no-return-assign
            "JS015", // no-self-compare
        ]
    }
}
