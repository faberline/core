use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::type_refactoring::multilang::{MultiLangRefactorer, RefactorLanguage};
use crate::domain::type_refactoring::request::RefactorRequest;
use crate::domain::type_refactoring::result::{DiagnosticLevel, RefactorResult, TextEdit};

impl MultiLangRefactorer {
    /// Extract function (Rust)
    pub(super) fn extract_function_rust(
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

        if RefactorLanguage::Rust.is_keyword(name) {
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

        // Build parameters with type annotations (required in Rust)
        let params: Vec<String> = data_flow.external_vars.clone();
        let params_str = params
            .iter()
            .map(|p| {
                if data_flow.mutable_vars.contains(p) {
                    format!("{}: &mut _", p)
                } else {
                    format!("{}: &_", p)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        // Build function body
        let body_lines: Vec<String> = selected_code
            .lines()
            .map(|line| format!("    {}", line))
            .collect();
        let mut body = body_lines.join("\n");

        // Add return if needed
        if !data_flow.returned_vars.is_empty() {
            let return_vars = data_flow.returned_vars.join(", ");
            if data_flow.returned_vars.len() == 1 {
                body.push_str(&format!("\n    {}", return_vars));
            } else {
                body.push_str(&format!("\n    ({})", return_vars));
            }
        }

        // Generate function
        let func_def = format!("fn {}({}) {{\n{}\n}}\n\n", name, params_str, body);

        // Generate call
        let call_str = if params.is_empty() {
            if data_flow.returned_vars.is_empty() {
                format!("{}();", name)
            } else if data_flow.returned_vars.len() == 1 {
                format!("let {} = {}();", data_flow.returned_vars[0], name)
            } else {
                format!("let ({}) = {}();", data_flow.returned_vars.join(", "), name)
            }
        } else {
            let call_params = params
                .iter()
                .map(|p| {
                    if data_flow.mutable_vars.contains(p) {
                        format!("&mut {}", p)
                    } else {
                        format!("&{}", p)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            if data_flow.returned_vars.is_empty() {
                format!("{}({});", name, call_params)
            } else if data_flow.returned_vars.len() == 1 {
                format!(
                    "let {} = {}({});",
                    data_flow.returned_vars[0], name, call_params
                )
            } else {
                format!(
                    "let ({}) = {}({});",
                    data_flow.returned_vars.join(", "),
                    name,
                    call_params
                )
            }
        };

        // Find insertion point
        let insert_pos = self.find_rust_insertion_point(source, request.span);

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
                "Extracted Rust function '{}' with {} parameter(s)",
                name,
                params.len()
            ),
            Some(request.file.clone()),
            Some(request.span),
        );

        result
    }

    /// Find insertion point for Rust
    fn find_rust_insertion_point(&self, source: &str, span: Span) -> usize {
        let selection_pos = span.start;
        let mut current_pos = 0;
        let mut function_start_pos = 0;

        for line in source.lines() {
            let line_end = current_pos + line.len();

            if current_pos > selection_pos {
                break;
            }

            let trimmed = line.trim_start();
            if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                function_start_pos = current_pos;
            }

            current_pos = line_end + 1;
        }

        // Find end of current function (matching braces)
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
