use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::type_refactoring::engine::{DataFlow, RefactoringEngine};

impl RefactoringEngine {
    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Analyze data flow for a specific span using text-based analysis.
    pub(super) fn analyze_data_flow_simple(&self, span: Span, source: &str) -> DataFlow {
        let selected_code = &source[span.start..span.end];

        // Simple regex-based approach for Python identifiers
        let mut used_vars = std::collections::HashSet::new();
        let mut defined_vars = std::collections::HashSet::new();

        // Find all assignment targets (defined variables)
        for line in selected_code.lines() {
            if let Some(eq_pos) = line.find('=') {
                let left_side = &line[..eq_pos].trim();
                // Simple case: single identifier assignment
                if left_side.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && !left_side.is_empty()
                {
                    defined_vars.insert(left_side.to_string());
                }

                // Right side contains used variables
                let right_side = &line[eq_pos + 1..];
                self.extract_identifiers_from_text(right_side, &mut used_vars);
            } else {
                // No assignment, just extract identifiers
                self.extract_identifiers_from_text(line, &mut used_vars);
            }
        }

        // External vars are used but not defined
        let external_vars: Vec<String> = used_vars
            .iter()
            .filter(|v| !defined_vars.contains(*v))
            .filter(|v| !self.is_builtin(v))
            .cloned()
            .collect();

        // Variables defined in the selection (potential return values)
        let returned: Vec<String> = defined_vars.iter().cloned().collect();

        DataFlow {
            external_vars,
            returned_vars: returned,
        }
    }

    /// Extract identifiers from text using simple pattern matching.
    fn extract_identifiers_from_text(
        &self,
        text: &str,
        identifiers: &mut std::collections::HashSet<String>,
    ) {
        let mut current_id = String::new();
        for ch in text.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                current_id.push(ch);
            } else {
                if let Some(first_char) = current_id.chars().next() {
                    if !first_char.is_numeric() {
                        identifiers.insert(current_id.clone());
                    }
                }
                current_id.clear();
            }
        }
        // Don't forget last identifier
        if let Some(first_char) = current_id.chars().next() {
            if !first_char.is_numeric() {
                identifiers.insert(current_id);
            }
        }
    }

    /// Check if an identifier is a Python builtin.
    fn is_builtin(&self, name: &str) -> bool {
        let builtins = [
            "print",
            "len",
            "range",
            "str",
            "int",
            "float",
            "bool",
            "list",
            "dict",
            "tuple",
            "set",
            "abs",
            "all",
            "any",
            "bin",
            "chr",
            "ord",
            "hex",
            "oct",
            "max",
            "min",
            "sum",
            "sorted",
            "reversed",
            "enumerate",
            "zip",
            "map",
            "filter",
            "open",
            "input",
            "type",
            "isinstance",
            "issubclass",
            "callable",
            "hasattr",
            "getattr",
            "setattr",
            "delattr",
            "dir",
            "vars",
            "globals",
            "locals",
            "super",
            "staticmethod",
            "classmethod",
            "property",
        ];
        builtins.contains(&name)
    }

    /// Get indentation at a specific byte position.
    pub(super) fn get_indent_at_position_static(source: &str, byte_pos: usize) -> String {
        // Find the line number containing this byte position
        let mut current_pos = 0;
        for line in source.lines() {
            let line_end = current_pos + line.len();

            // Check if byte_pos is in this line
            if byte_pos >= current_pos && byte_pos <= line_end {
                let indent_len = line.len() - line.trim_start().len();
                return " ".repeat(indent_len);
            }

            // +1 for newline character
            current_pos = line_end + 1;
        }

        String::new()
    }

    /// Find insertion point for a new function definition.
    pub(super) fn find_function_insertion_point(&self, source: &str, span: Span) -> usize {
        // Find the end of the current function or class containing the selection
        // Strategy:
        // 1. Find the line containing the selection
        // 2. Search backwards for "def " or "class " at the beginning of a line
        // 3. Find the end of that definition (next def/class at same indentation or EOF)

        let selection_pos = span.start;

        // Find the start line of current function/class
        let mut current_pos = 0;
        let mut function_start_pos = 0;
        let mut function_indent = 0;

        for line in source.lines() {
            let line_end = current_pos + line.len();

            // Check if we've passed the selection
            if current_pos > selection_pos {
                break;
            }

            // Check if this line starts a function or class
            let trimmed = line.trim_start();
            if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
                function_start_pos = current_pos;
                function_indent = line.len() - trimmed.len();
            }

            current_pos = line_end + 1; // +1 for newline
        }

        // Now find the end of this function/class
        // Look for next def/class at same or lower indentation
        current_pos = function_start_pos;
        let mut found_start = false;

        for line in source[function_start_pos..].lines() {
            let line_end = current_pos + line.len();

            if !found_start {
                found_start = true;
                current_pos = line_end + 1;
                continue;
            }

            let trimmed = line.trim_start();
            let line_indent = line.len() - trimmed.len();

            // Check if we found another def/class at same or lower indentation
            if (trimmed.starts_with("def ") || trimmed.starts_with("class "))
                && line_indent <= function_indent
            {
                // Insert before this line
                return current_pos;
            }

            // Check for end of file or empty lines
            if line.trim().is_empty() {
                current_pos = line_end + 1;
                continue;
            }

            current_pos = line_end + 1;
        }

        // If we didn't find another function, insert at end of file
        source.len()
    }

    /// Find insertion point for a new method definition within a class.
    pub(super) fn find_method_insertion_point(&self, source: &str, span: Span) -> usize {
        // Similar to function insertion, but finds the end of the current class
        // For now, reuse the same logic
        self.find_function_insertion_point(source, span)
    }
}
