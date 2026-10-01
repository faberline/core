use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::semantic_search::query::{SearchKind, SearchQuery, SearchScope};
use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::domain::type_refactoring::request::RefactorRequest;
use crate::domain::type_refactoring::result::{DiagnosticLevel, RefactorResult, TextEdit};

impl RefactoringEngine {
    /// Rename a symbol across files.
    pub(super) fn rename_symbol(
        &mut self,
        request: &RefactorRequest,
        new_name: &str,
        source: &str,
    ) -> RefactorResult {
        let mut result = RefactorResult::empty();

        // Extract the old symbol name from the source
        let old_name = &source[request.span.start..request.span.end];

        // Validate new name
        if old_name == new_name {
            result.add_diagnostic(
                DiagnosticLevel::Info,
                "New name is the same as the old name",
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

        if new_name.is_empty() {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                "New name cannot be empty",
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

        // Basic name validation (simplified - just check it's a valid identifier)
        if !new_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                "New name must be a valid identifier",
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

        // Check if new name is a Python keyword
        let python_keywords = [
            "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
            "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
            "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
            "return", "try", "while", "with", "yield",
        ];

        if python_keywords.contains(&new_name) {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                format!(
                    "'{}' is a reserved keyword and cannot be used as a variable name",
                    new_name
                ),
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

        // For simplified implementation: just rename in current file
        // In a full implementation, would use semantic search to find all references
        let query = SearchQuery {
            kind: SearchKind::Usages {
                symbol: old_name.to_string(),
                file: request.file.clone(),
            },
            scope: SearchScope::Project,
            max_results: 1000,
        };

        let search_result = self.search_engine.search(&query);

        // If no matches found in index, do simple text-based search in current file
        if search_result.matches.is_empty() {
            // Simple approach: find all occurrences of the old name in the current file
            let mut pos = 0;
            while let Some(found_pos) = source[pos..].find(old_name) {
                let absolute_pos = pos + found_pos;

                // Create a text edit for this occurrence
                result.add_edit(
                    request.file.clone(),
                    TextEdit {
                        span: Span::new(absolute_pos, absolute_pos + old_name.len()),
                        new_text: new_name.to_string(),
                    },
                );

                pos = absolute_pos + old_name.len();
            }
        } else {
            // Use search results
            for search_match in &search_result.matches {
                result.add_edit(
                    search_match.file.clone(),
                    TextEdit {
                        span: search_match.span,
                        new_text: new_name.to_string(),
                    },
                );
            }
        }

        if !result.has_changes() {
            result.add_diagnostic(
                DiagnosticLevel::Warning,
                format!("No occurrences of '{}' found", old_name),
                Some(request.file.clone()),
                Some(request.span),
            );
        } else {
            result.add_diagnostic(
                DiagnosticLevel::Info,
                format!(
                    "Renamed '{}' to '{}' ({} occurrences)",
                    old_name,
                    new_name,
                    result.file_edits.values().map(|v| v.len()).sum::<usize>()
                ),
                Some(request.file.clone()),
                Some(request.span),
            );
        }

        result
    }
}
