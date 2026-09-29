use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::type_refactoring::multilang::{MultiLangRefactorer, RefactorLanguage};
use crate::domain::type_refactoring::request::RefactorRequest;
use crate::domain::type_refactoring::result::{DiagnosticLevel, RefactorResult, TextEdit};

impl MultiLangRefactorer {
    /// Extract function (TypeScript)
    pub(super) fn extract_function_typescript(
        &self,
        request: &RefactorRequest,
        name: &str,
        source: &str,
    ) -> RefactorResult {
        let mut result = RefactorResult::empty();

        // Validate function name
        if name.is_empty() {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                "Function name cannot be empty",
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

        if RefactorLanguage::TypeScript.is_keyword(name) {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                format!("'{}' is a reserved keyword", name),
                Some(request.file.clone()),
                Some(request.span),
            );
            return result;
        }

        // Analyze data flow
        let data_flow = self.analyze_data_flow(request.span, source);
        let selected_code = &source[request.span.start..request.span.end];

        // Build parameters
        let params: Vec<String> = data_flow.external_vars.clone();
        let params_str = if request.options.add_type_annotations {
            params
                .iter()
                .map(|p| format!("{}: any", p))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            params.join(", ")
        };

        // Build function body
        let body_lines: Vec<String> = selected_code
            .lines()
            .map(|line| format!("  {}", line))
            .collect();
        let mut body = body_lines.join("\n");

        // Add return if needed
        if !data_flow.returned_vars.is_empty() {
            let return_vars = data_flow.returned_vars.join(", ");
            if data_flow.returned_vars.len() == 1 {
                body.push_str(&format!("\n  return {};", return_vars));
            } else {
                body.push_str(&format!("\n  return {{ {} }};", return_vars));
            }
        }

        // Generate function
        let return_type = if request.options.add_type_annotations {
            if data_flow.returned_vars.is_empty() {
                ": void"
            } else {
                ": any"
            }
        } else {
            ""
        };

        let func_def = format!(
            "function {}({}){} {{\n{}\n}}\n\n",
            name, params_str, return_type, body
        );

        // Generate call
        let call_str = if params.is_empty() {
            if data_flow.returned_vars.is_empty() {
                format!("{}();", name)
            } else if data_flow.returned_vars.len() == 1 {
                format!("const {} = {}();", data_flow.returned_vars[0], name)
            } else {
                format!(
                    "const {{ {} }} = {}();",
                    data_flow.returned_vars.join(", "),
                    name
                )
            }
        } else {
            let call_params = params.join(", ");
            if data_flow.returned_vars.is_empty() {
                format!("{}({});", name, call_params)
            } else if data_flow.returned_vars.len() == 1 {
                format!(
                    "const {} = {}({});",
                    data_flow.returned_vars[0], name, call_params
                )
            } else {
                format!(
                    "const {{ {} }} = {}({});",
                    data_flow.returned_vars.join(", "),
                    name,
                    call_params
                )
            }
        };

        // Find insertion point
        let insert_pos = self.find_ts_insertion_point(source, request.span);

        result.add_edit(
            request.file.clone(),
            TextEdit {
                span: Span::new(insert_pos, insert_pos),
                new_text: func_def,
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
                "Extracted TypeScript function '{}' with {} parameter(s)",
                name,
                params.len()
            ),
            Some(request.file.clone()),
            Some(request.span),
        );

        result
    }

    /// Find insertion point for TypeScript
    fn find_ts_insertion_point(&self, source: &str, span: Span) -> usize {
        let selection_pos = span.start;
        let mut current_pos = 0;
        let mut function_start_pos = 0;
        let mut _function_indent = 0;

        for line in source.lines() {
            let line_end = current_pos + line.len();

            if current_pos > selection_pos {
                break;
            }

            let trimmed = line.trim_start();
            if trimmed.starts_with("function ")
                || trimmed.contains("=>") && trimmed.contains("const ")
            {
                function_start_pos = current_pos;
                _function_indent = line.len() - trimmed.len();
            }

            current_pos = line_end + 1;
        }

        // Find end of current function
        current_pos = function_start_pos;
        let mut brace_count = 0;
        let mut found_start = false;

        for line in source[function_start_pos..].lines() {
            for ch in line.chars() {
                if ch == '{' {
                    brace_count += 1;
                    found_start = true;
                } else if ch == '}' {
                    brace_count -= 1;
                    if found_start && brace_count == 0 {
                        return current_pos + line.len() + 1;
                    }
                }
            }
            current_pos += line.len() + 1;
        }

        source.len()
    }
}
