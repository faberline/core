use crate::diagnostic::{Diagnostic, DiagnosticCategory};
use crate::domain::check::lint_config::LintConfig;
use crate::domain::syntax::parsed_file::NodeRange;
use crate::syntax::{Language, ParsedFile};

mod basic_rules;
mod stylelint_rules;

/// CSS checker
pub struct CssChecker;

impl CssChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CssChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl super::checker::Checker for CssChecker {
    fn language(&self) -> Language {
        Language::Css
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check for syntax errors from tree-sitter
        if file.has_errors {
            file.walk(|node, _depth| {
                if node.is_error() || node.is_missing() {
                    diagnostics.push(Diagnostic::error(
                        node.to_range(),
                        "CSS000",
                        DiagnosticCategory::Syntax,
                        "Syntax error",
                    ));
                }
                true
            });
        }

        // Run all checks
        diagnostics.extend(self.check_duplicate_selectors(file));
        diagnostics.extend(self.check_important_usage(file));
        diagnostics.extend(self.check_import_usage(file));
        diagnostics.extend(self.check_empty_rules(file));
        diagnostics.extend(self.check_universal_selector(file));
        diagnostics.extend(self.check_no_id_selectors(file));
        diagnostics.extend(self.check_shorthand_overrides(file));
        diagnostics.extend(self.check_z_index_max(file));
        diagnostics.extend(self.check_invalid_hex_color(file));
        diagnostics.extend(self.check_font_family_generic(file));
        diagnostics.extend(self.check_duplicate_properties(file));
        diagnostics.extend(self.check_shorthand_after_longhand(file));
        diagnostics.extend(self.check_descending_specificity(file));
        diagnostics.extend(self.check_unknown_units(file));
        diagnostics.extend(self.check_empty_at_rule_blocks(file));

        diagnostics
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "CSS000", // Syntax error
            "CSS001", // Duplicate selectors
            "CSS002", // !important usage
            "CSS003", // @import usage
            "CSS004", // Empty rule sets
            "CSS005", // Universal selector *
            "CSS006", // no-id-selectors
            "CSS007", // shorthand-property-overrides
            "CSS008", // z-index-max
            "CSS009", // color-no-invalid-hex
            "CSS010", // font-family-no-missing-generic
            "CSS011", // no-duplicate-properties
            "CSS012", // declaration-block-no-shorthand-override
            "CSS013", // no-descending-specificity
            "CSS014", // unit-no-unknown
            "CSS015", // block-no-empty
        ]
    }
}
