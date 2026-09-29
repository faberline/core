use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::domain::type_refactoring::request::RefactorRequest;
use crate::domain::type_refactoring::result::{DiagnosticLevel, RefactorResult, TextEdit};

impl RefactoringEngine {
    /// Inline a symbol's definition.
    pub(super) fn inline_symbol(
        &mut self,
        request: &RefactorRequest,
        source: &str,
    ) -> RefactorResult {
        let mut result = RefactorResult::empty();

        // Get or populate AST
        if let Err(e) = self.populate_ast_cache(&request.file, source) {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                format!("Failed to parse file: {}", e),
                Some(request.file.clone()),
                None,
            );
            return result;
        }

        // Extract the symbol name to inline
        let symbol_name = &source[request.span.start..request.span.end];

        // Simplified implementation: Find the definition and replace all usages
        // Look for pattern: "symbol_name = expression"
        let mut definition_value = None;
        let mut definition_span = None;

        // Simple pattern matching for variable assignment
        if let Some(def_pos) = source.find(&format!("{} = ", symbol_name)) {
            let start = def_pos;
            // Find the end of the line (or end of file if no newline)
            let rest = &source[start..];
            let newline_pos = rest.find('\n').unwrap_or(rest.len());
            let end = start + newline_pos;
            let line = &source[start..end];

            // Extract the value part (after "symbol_name = ")
            if let Some(eq_pos) = line.find(" = ") {
                let value_start = start + eq_pos + 3;
                definition_value = Some(source[value_start..end].trim().to_string());
                // Include newline only if it exists
                let span_end = if newline_pos < rest.len() {
                    end + 1
                } else {
                    end
                };
                definition_span = Some(Span::new(start, span_end));
            }
        }

        let (def_value, def_span) = match (definition_value, definition_span) {
            (Some(value), Some(span)) => (value, span),
            _ => {
                result.add_diagnostic(
                    DiagnosticLevel::Error,
                    format!("Could not find definition for '{}'", symbol_name),
                    Some(request.file.clone()),
                    Some(request.span),
                );
                return result;
            }
        };

        // Find all usages of the symbol (excluding the definition)
        let mut usages = Vec::new();
        let mut pos = 0;
        while let Some(found_pos) = source[pos..].find(symbol_name) {
            let absolute_pos = pos + found_pos;

            // Skip if this is the definition itself
            if absolute_pos >= def_span.start && absolute_pos < def_span.end {
                pos = absolute_pos + symbol_name.len();
                continue;
            }

            // Check if it's a complete identifier (not part of another word)
            let before_ok = absolute_pos == 0
                || !source
                    .chars()
                    .nth(absolute_pos - 1)
                    .unwrap_or(' ')
                    .is_alphanumeric();
            let after_pos = absolute_pos + symbol_name.len();
            let after_ok = after_pos >= source.len()
                || !source
                    .chars()
                    .nth(after_pos)
                    .unwrap_or(' ')
                    .is_alphanumeric();

            if before_ok && after_ok {
                usages.push(Span::new(absolute_pos, absolute_pos + symbol_name.len()));
            }

            pos = absolute_pos + symbol_name.len();
        }

        if usages.is_empty() {
            result.add_diagnostic(
                DiagnosticLevel::Warning,
                format!("No usages of '{}' found to inline", symbol_name),
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

        // Store count before iterating
        let usage_count = usages.len();

        // Replace all usages with the definition value
        for usage_span in usages {
            result.add_edit(
                request.file.clone(),
                TextEdit {
                    span: usage_span,
                    new_text: def_value.clone(),
                },
            );
        }

        // Remove the definition line
        result.add_edit(
            request.file.clone(),
            TextEdit {
                span: def_span,
                new_text: String::new(),
            },
        );

        result.add_diagnostic(
            DiagnosticLevel::Info,
            format!("Inlined '{}' ({} usages)", symbol_name, usage_count),
            Some(request.file.clone()),
            Some(request.span),
        );

        result
    }
}
