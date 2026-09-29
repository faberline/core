use super::PythonChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory};
use crate::semantic::ScopeAnalyzer;
use crate::syntax::ParsedFile;

impl PythonChecker {
    /// Perform scope analysis and return diagnostics for unused/redeclared variables
    pub(super) fn check_scope_issues(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut analyzer = ScopeAnalyzer::new();
        analyzer.analyze(file);

        // PY103: Unused variable/parameter
        for symbol in analyzer.unused_symbols() {
            let message = match symbol.kind {
                crate::semantic::ScopeSymbolKind::Parameter => {
                    format!("Unused parameter: '{}'", symbol.name)
                }
                _ => {
                    format!("Unused variable: '{}'", symbol.name)
                }
            };
            diagnostics.push(Diagnostic::warning(
                symbol.defined_at.clone(),
                "PY103",
                DiagnosticCategory::Names,
                message,
            ));
        }

        // PY106: Variable redeclaration (assigned multiple times without use)
        for symbol in analyzer.redeclared_symbols() {
            diagnostics.push(Diagnostic::new(
                symbol.defined_at.clone(),
                crate::diagnostic::DiagnosticSeverity::Hint,
                "PY106",
                DiagnosticCategory::Names,
                format!("Variable '{}' is assigned multiple times", symbol.name),
            ));
        }

        diagnostics
    }
}
