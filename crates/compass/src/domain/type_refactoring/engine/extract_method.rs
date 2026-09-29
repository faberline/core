use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::domain::type_refactoring::request::RefactorRequest;
use crate::domain::type_refactoring::result::{DiagnosticLevel, RefactorResult, TextEdit};

impl RefactoringEngine {
    /// Extract selection into a new method.
    pub(super) fn extract_method(
        &mut self,
        request: &RefactorRequest,
        name: &str,
        source: &str,
    ) -> RefactorResult {
        let mut result = RefactorResult::empty();

        // Validate method name
        if name.is_empty() {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                "Method name cannot be empty",
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                "Method name must be a valid identifier",
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

        // Perform data flow analysis using simple text-based approach
        let data_flow = self.analyze_data_flow_simple(request.span, source);

        // Extract the selected code
        let selected_code = &source[request.span.start..request.span.end];

        // Build parameter list from external variables (excluding 'self')
        let mut params: Vec<String> = data_flow.external_vars.clone();
        params.retain(|p| p != "self"); // Remove self if it appears as external var

        // Build method parameters: self + other params
        let mut method_params = vec!["self".to_string()];

        // Add type annotations if requested
        if request.options.add_type_annotations {
            for p in &params {
                method_params.push(format!("{}: Any", p));
            }
        } else {
            method_params.extend(params.clone());
        }

        let params_str = method_params.join(", ");

        // Build method body (with proper indentation for class method)
        let body_lines: Vec<String> = selected_code
            .lines()
            .map(|line| format!("        {}", line))
            .collect();

        let mut body = body_lines.join("\n");

        // Add return statement if variables are defined and need to be returned
        if !data_flow.returned_vars.is_empty() {
            let return_vars = data_flow.returned_vars.join(", ");
            if data_flow.returned_vars.len() == 1 {
                body.push_str(&format!("\n        return {}", return_vars));
            } else {
                body.push_str(&format!("\n        return ({})", return_vars));
            }
        }

        // Generate method definition with class indentation and optional type annotations
        let method_def =
            if request.options.add_type_annotations && !data_flow.returned_vars.is_empty() {
                if data_flow.returned_vars.len() == 1 {
                    format!("    def {}({}) -> Any:\n{}\n\n", name, params_str, body)
                } else {
                    format!("    def {}({}) -> tuple:\n{}\n\n", name, params_str, body)
                }
            } else {
                format!("    def {}({}):\n{}\n\n", name, params_str, body)
            };

        // Generate method call
        let call_str = if params.is_empty() {
            if data_flow.returned_vars.is_empty() {
                format!("self.{}()", name)
            } else if data_flow.returned_vars.len() == 1 {
                format!("{} = self.{}()", data_flow.returned_vars[0], name)
            } else {
                format!("{} = self.{}()", data_flow.returned_vars.join(", "), name)
            }
        } else {
            let call_params = params.join(", ");
            if data_flow.returned_vars.is_empty() {
                format!("self.{}({})", name, call_params)
            } else if data_flow.returned_vars.len() == 1 {
                format!(
                    "{} = self.{}({})",
                    data_flow.returned_vars[0], name, call_params
                )
            } else {
                format!(
                    "{} = self.{}({})",
                    data_flow.returned_vars.join(", "),
                    name,
                    call_params
                )
            }
        };

        // Find appropriate insertion point (after current class/method)
        let insert_pos = self.find_method_insertion_point(source, request.span);

        // Create edits
        result.add_edit(
            request.file.clone(),
            TextEdit {
                span: Span::new(insert_pos, insert_pos),
                new_text: method_def,
            },
        );

        result.add_edit(
            request.file.clone(),
            TextEdit {
                span: request.span,
                new_text: call_str,
            },
        );

        result.add_diagnostic(
            DiagnosticLevel::Info,
            format!(
                "Extracted method '{}' with {} parameter(s) (plus self) and {} return value(s)",
                name,
                params.len(),
                data_flow.returned_vars.len()
            ),
            Some(request.file.clone()),
            Some(request.span),
        );

        result
    }
}
