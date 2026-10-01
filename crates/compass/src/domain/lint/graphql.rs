use super::checker::Checker;
use crate::diagnostic::{Diagnostic, Position, Range};
use crate::domain::check::lint_config::LintConfig;
use crate::syntax::{Language, ParsedFile};

mod ast_rules;
mod line_rules;

pub struct GraphqlChecker;

impl GraphqlChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GraphqlChecker {
    fn default() -> Self {
        Self::new()
    }
}

fn lr(line: u32) -> Range {
    Range::new(Position::new(line, 0), Position::new(line, u32::MAX))
}

impl Checker for GraphqlChecker {
    fn language(&self) -> Language {
        Language::GraphQL
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        // Use AST-based checks when the file has a real tree-sitter tree
        // (language == GraphQL means parser.parse() succeeded with the real grammar).
        // Fall back to line-based for dummy trees (line_based() or has_errors).
        if file.language == Language::GraphQL && !file.has_errors && !file.is_line_based {
            let mut d = Vec::new();
            d.extend(self.ast_check_syntax(file));
            d.extend(self.ast_check_undefined_types(file));
            d.extend(self.ast_check_deprecated(file));
            d.extend(self.ast_check_deep_nesting(file));
            d.extend(self.ast_check_missing_descriptions(file));
            d.extend(self.ast_check_unused_fragments(file));
            d.extend(self.ast_check_duplicate_fields(file));
            d
        } else {
            let mut d = Vec::new();
            d.extend(self.lb_check_syntax(&file.source));
            d.extend(self.lb_check_undefined_types(&file.source));
            d.extend(self.lb_check_deprecated(&file.source));
            d.extend(self.lb_check_deep_nesting(&file.source));
            d.extend(self.lb_check_missing_descriptions(&file.source));
            d.extend(self.lb_check_unused_fragments(&file.source));
            d.extend(self.lb_check_duplicate_fields(&file.source));
            d
        }
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "GQ001", "GQ002", "GQ003", "GQ004", "GQ005", "GQ006", "GQ007",
        ]
    }
}

#[cfg(test)]
mod tests;
