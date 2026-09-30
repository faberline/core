use super::checker::Checker;
use crate::diagnostic::{Diagnostic, Position, Range};
use crate::domain::check::lint_config::LintConfig;
use crate::syntax::{Language, ParsedFile};

mod ast_rules;
mod line_rules;

pub struct ProtoChecker;

impl ProtoChecker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProtoChecker {
    fn default() -> Self {
        Self::new()
    }
}

fn lr(line: u32) -> Range {
    Range::new(Position::new(line, 0), Position::new(line, u32::MAX))
}

fn field_number(line: &str) -> Option<String> {
    let after = line[line.find('=')? + 1..]
        .trim()
        .trim_end_matches(';')
        .trim();
    if after.chars().all(|c| c.is_ascii_digit()) {
        Some(after.to_string())
    } else {
        None
    }
}

fn field_name(line: &str) -> Option<&str> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 && parts.iter().any(|p| p.contains('=')) {
        Some(parts[1])
    } else {
        None
    }
}

fn import_path(line: &str) -> Option<String> {
    let after = line.trim().strip_prefix("import ")?;
    let after = after.strip_prefix("public ").unwrap_or(after);
    let p = after
        .trim()
        .trim_matches('"')
        .trim_end_matches(';')
        .trim_matches('"');
    if p.is_empty() {
        None
    } else {
        Some(p.to_string())
    }
}

fn to_pascal(s: &str) -> String {
    s.split(|c: char| c == '_' || c == '-')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(ch) => ch.to_uppercase().to_string() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect()
}

fn is_snake_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !s.starts_with('_')
        && !s.ends_with('_')
        && !s.contains("__")
}

impl Checker for ProtoChecker {
    fn language(&self) -> Language {
        Language::Proto
    }

    fn check(&self, file: &ParsedFile, _config: &LintConfig) -> Vec<Diagnostic> {
        let mut d = Vec::new();

        // R3: AST-based syntax error detection when a real tree is present;
        // semantic checks use AST node walking for field numbers, services, etc.
        if file.language == Language::Proto && !file.has_errors && !file.is_line_based {
            // PB001 via AST: tree-sitter parse errors
            d.extend(self.ast_check_syntax(file));
            // Semantic checks using AST node matching
            d.extend(self.ast_check_duplicate_field_numbers(file));
            d.extend(self.ast_check_empty_service(file));
            d.extend(self.ast_check_missing_package(file));
            // Field naming and reserved checks still use line-based patterns
            d.extend(self.check_reserved_fields(&file.source));
            d.extend(self.check_unused_imports(&file.source));
            d.extend(self.check_field_naming(&file.source));
        } else {
            // Line-based fallback
            d.extend(self.check_syntax(&file.source));
            d.extend(self.check_duplicate_field_numbers(&file.source));
            d.extend(self.check_reserved_fields(&file.source));
            d.extend(self.check_missing_package(&file.source));
            d.extend(self.check_empty_service(&file.source));
            d.extend(self.check_unused_imports(&file.source));
            d.extend(self.check_field_naming(&file.source));
        }

        d
    }

    fn available_rules(&self) -> Vec<&'static str> {
        vec![
            "PB001", "PB002", "PB003", "PB004", "PB005", "PB006", "PB007",
        ]
    }
}

#[cfg(test)]
mod tests;
