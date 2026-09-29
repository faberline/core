use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::domain::type_refactoring::request::{RefactorRequest, SignatureChanges};
use crate::domain::type_refactoring::result::{DiagnosticLevel, RefactorResult, TextEdit};

impl RefactoringEngine {
    /// Change function signature.
    pub(super) fn change_signature(
        &mut self,
        request: &RefactorRequest,
        changes: &SignatureChanges,
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

        // Simplified implementation: Find function definition and update parameters
        let function_code = &source[request.span.start..request.span.end];

        // Find the opening parenthesis
        if let Some(open_paren) = function_code.find('(') {
            if let Some(close_paren) = function_code.find(')') {
                let func_name_part = &function_code[..open_paren];
                let after_params = &function_code[close_paren + 1..];

                // Build new parameters list
                let mut new_params = Vec::new();

                // Add new parameters
                for (param_name, type_ann, default) in &changes.new_params {
                    let param_str = if let Some(type_str) = type_ann {
                        if let Some(default_val) = default {
                            format!("{}: {} = {}", param_name, type_str, default_val)
                        } else {
                            format!("{}: {}", param_name, type_str)
                        }
                    } else if let Some(default_val) = default {
                        format!("{}={}", param_name, default_val)
                    } else {
                        param_name.clone()
                    };
                    new_params.push(param_str);
                }

                let new_params_str = new_params.join(", ");

                // Construct new function signature
                let new_signature =
                    format!("{}({}){}", func_name_part, new_params_str, after_params);

                // Replace the function signature
                result.add_edit(
                    request.file.clone(),
                    TextEdit {
                        span: request.span,
                        new_text: new_signature,
                    },
                );

                result.add_diagnostic(
                    DiagnosticLevel::Info,
                    format!(
                        "Changed function signature ({} parameters)",
                        new_params.len()
                    ),
                    Some(request.file.clone()),
                    Some(request.span),
                );
            } else {
                result.add_diagnostic(
                    DiagnosticLevel::Error,
                    "Could not find closing parenthesis",
                    Some(request.file.clone()),
                    Some(request.span),
                );
            }
        } else {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                "Could not find function parameters",
                Some(request.file.clone()),
                Some(request.span),
            );
        }

        result
    }
}
