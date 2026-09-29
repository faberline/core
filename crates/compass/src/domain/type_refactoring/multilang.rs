//! Multi-language refactoring support
//!

use std::collections::HashSet;
use std::path::PathBuf;

use crate::domain::ast_editing::mutable_ast::Span;
#[allow(unused_imports)]
use crate::domain::type_refactoring::{
    request::{RefactorOptions, RefactorRequest},
    result::{DiagnosticLevel, RefactorResult, TextEdit},
};

mod extract_python;
mod extract_rust;
mod extract_typescript;

/// Language for refactoring operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefactorLanguage {
    Python,
    TypeScript,
    Rust,
}

impl RefactorLanguage {
    /// Detect language from file extension
    pub fn from_path(path: &PathBuf) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        match ext {
            "py" => Some(Self::Python),
            "ts" | "tsx" | "js" | "jsx" => Some(Self::TypeScript),
            "rs" => Some(Self::Rust),
            _ => None,
        }
    }

    /// Get keywords for this language
    pub fn keywords(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &[
                "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
                "continue", "def", "del", "elif", "else", "except", "finally", "for", "from",
                "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass",
                "raise", "return", "try", "while", "with", "yield",
            ],
            Self::TypeScript => &[
                "break",
                "case",
                "catch",
                "class",
                "const",
                "continue",
                "debugger",
                "default",
                "delete",
                "do",
                "else",
                "enum",
                "export",
                "extends",
                "false",
                "finally",
                "for",
                "function",
                "if",
                "import",
                "in",
                "instanceof",
                "new",
                "null",
                "return",
                "super",
                "switch",
                "this",
                "throw",
                "true",
                "try",
                "typeof",
                "var",
                "void",
                "while",
                "with",
                "yield",
                "let",
                "static",
                "implements",
                "interface",
                "package",
                "private",
                "protected",
                "public",
                "async",
                "await",
                "type",
                "as",
            ],
            Self::Rust => &[
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
                "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
                "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
                "super", "trait", "true", "type", "unsafe", "use", "where", "while",
            ],
        }
    }

    /// Check if name is a keyword
    pub fn is_keyword(&self, name: &str) -> bool {
        self.keywords().contains(&name)
    }
}

/// Multi-language refactoring engine
pub struct MultiLangRefactorer;

impl MultiLangRefactorer {
    pub fn new() -> Self {
        Self
    }

    /// Extract function for any supported language
    pub fn extract_function(
        &self,
        request: &RefactorRequest,
        name: &str,
        source: &str,
    ) -> RefactorResult {
        let lang = match RefactorLanguage::from_path(&request.file) {
            Some(l) => l,
            None => {
                let mut result = RefactorResult::empty();
                result.add_diagnostic(
                    DiagnosticLevel::Error,
                    "Unsupported file type for refactoring",
                    Some(request.file.clone()),
                    None,
                );
                return result;
            }
        };

        match lang {
            RefactorLanguage::Python => self.extract_function_python(request, name, source),
            RefactorLanguage::TypeScript => self.extract_function_typescript(request, name, source),
            RefactorLanguage::Rust => self.extract_function_rust(request, name, source),
        }
    }

    /// Simple data flow analysis
    fn analyze_data_flow(&self, span: Span, source: &str) -> DataFlowResult {
        let selected = &source[span.start..span.end];

        // Find identifiers in selection
        let mut defined = HashSet::new();
        let mut used = HashSet::new();
        let mut mutated = HashSet::new();

        // Simple regex-like patterns
        for word in selected.split(|c: char| !c.is_alphanumeric() && c != '_') {
            if !word.is_empty()
                && word
                    .chars()
                    .next()
                    .map(|c| c.is_alphabetic() || c == '_')
                    .unwrap_or(false)
            {
                used.insert(word.to_string());
            }
        }

        // Find assignments
        for line in selected.lines() {
            let trimmed = line.trim();
            // Python: x = ...
            // Rust: let x = ... or let mut x = ...
            // TS: const x = ... or let x = ... or var x = ...
            if let Some(idx) = trimmed.find('=') {
                let before = &trimmed[..idx].trim();
                // Handle let/const/var/mut
                let var_part = before
                    .trim_start_matches("let ")
                    .trim_start_matches("const ")
                    .trim_start_matches("var ")
                    .trim_start_matches("mut ")
                    .trim();

                if !var_part.is_empty() {
                    for word in var_part.split(|c: char| !c.is_alphanumeric() && c != '_') {
                        if !word.is_empty() {
                            defined.insert(word.to_string());
                        }
                    }
                }
            }

            // Check for mutation (&mut, += etc)
            if trimmed.contains("&mut ") || trimmed.contains("+=") || trimmed.contains("-=") {
                for word in trimmed.split(|c: char| !c.is_alphanumeric() && c != '_') {
                    if !word.is_empty() {
                        mutated.insert(word.to_string());
                    }
                }
            }
        }

        // External vars = used but not defined in selection
        let external_vars: Vec<String> = used
            .difference(&defined)
            .filter(|v| {
                !["self", "this", "true", "false", "None", "null", "undefined"]
                    .contains(&v.as_str())
            })
            .cloned()
            .collect();

        // Variables defined that may need to be returned
        let returned_vars: Vec<String> = defined.iter().cloned().collect();

        DataFlowResult {
            external_vars,
            returned_vars,
            mutable_vars: mutated,
        }
    }

    /// Cross-file rename for any language
    pub fn rename_symbol(
        &self,
        file: &PathBuf,
        old_name: &str,
        new_name: &str,
        _source: &str,
        all_usages: &[(PathBuf, Span)],
    ) -> RefactorResult {
        let mut result = RefactorResult::empty();

        let lang = match RefactorLanguage::from_path(file) {
            Some(l) => l,
            None => {
                result.add_diagnostic(
                    DiagnosticLevel::Error,
                    "Unsupported file type",
                    Some(file.clone()),
                    None,
                );
                return result;
            }
        };

        // Validate new name
        if new_name.is_empty() {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                "New name cannot be empty",
                Some(file.clone()),
                None,
            );
            return result;
        }

        if lang.is_keyword(new_name) {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                format!("'{}' is a reserved keyword", new_name),
                Some(file.clone()),
                None,
            );
            return result;
        }

        // Create edits for all usages
        for (usage_file, span) in all_usages {
            result.add_edit(
                usage_file.clone(),
                TextEdit {
                    span: *span,
                    new_text: new_name.to_string(),
                },
            );
        }

        result.add_diagnostic(
            DiagnosticLevel::Info,
            format!(
                "Renamed '{}' to '{}' across {} file(s)",
                old_name,
                new_name,
                result.file_edits.len()
            ),
            Some(file.clone()),
            None,
        );

        result
    }
}

impl Default for MultiLangRefactorer {
    fn default() -> Self {
        Self::new()
    }
}

/// Data flow analysis result
struct DataFlowResult {
    external_vars: Vec<String>,
    returned_vars: Vec<String>,
    mutable_vars: HashSet<String>,
}

#[cfg(test)]
mod tests;
