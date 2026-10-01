use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::domain::type_refactoring::request::RefactorRequest;
use crate::domain::type_refactoring::result::{DiagnosticLevel, RefactorResult, TextEdit};

impl RefactoringEngine {
    /// Extract expression into a variable.
    pub(super) fn extract_variable(
        &mut self,
        request: &RefactorRequest,
        name: &str,
        source: &str,
    ) -> RefactorResult {
        let mut result = RefactorResult::empty();

        // Validate variable name
        if name.is_empty() {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                "Variable name cannot be empty",
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                "Variable name must be a valid identifier",
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

        // Check if name is a Python keyword
        let python_keywords = [
            "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
            "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
            "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
            "return", "try", "while", "with", "yield",
        ];

        if python_keywords.contains(&name) {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                format!("'{}' is a reserved keyword", name),
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

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

        // Extract the expression text
        let expr_text = &source[request.span.start..request.span.end].trim();

        // Get indentation at the current position
        let indent = Self::get_indent_at_position_static(source, request.span.start);

        // Generate variable assignment
        let assignment = format!("{}{} = {}\n", indent, name, expr_text);

        // Find line start position
        let line_start = Self::find_line_start(source, request.span.start);

        // Insert the assignment before the current line
        result.add_edit(
            request.file.clone(),
            TextEdit {
                span: Span::new(line_start, line_start),
                new_text: assignment,
            },
        );

        // Replace the expression with the variable name
        result.add_edit(
            request.file.clone(),
            TextEdit {
                span: request.span,
                new_text: name.to_string(),
            },
        );

        result.add_diagnostic(
            DiagnosticLevel::Info,
            format!("Extracted variable '{}'", name),
            Some(request.file.clone()),
            Some(request.span),
        );

        result
    }

    /// Find the start of a line containing a byte position.
    fn find_line_start(source: &str, pos: usize) -> usize {
        let mut line_start = 0;
        for (i, ch) in source.char_indices() {
            if i >= pos {
                break;
            }
            if ch == '\n' {
                line_start = i + 1;
            }
        }
        line_start
    }
}
