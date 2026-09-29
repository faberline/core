//! Rust-to-Python test body translation.

use super::{RustTest, TestExtractor};

impl TestExtractor {
    /// Convert a Rust test to Python test code
    pub fn translate_to_python(&self, test: &RustTest) -> String {
        let mut py_body = self.translate_rust_to_python(&test.body);

        // Indent the body
        py_body = py_body
            .lines()
            .map(|line| format!("    {}", line))
            .collect::<Vec<_>>()
            .join("\n");

        let decorator = if test.is_async {
            "@pytest.mark.asyncio\n"
        } else {
            ""
        };
        let async_kw = if test.is_async { "async " } else { "" };

        format!(
            r#"{decorator}{async_kw}def {name}():
    """Translated from: {source}:{line}"""
{body}
"#,
            decorator = decorator,
            async_kw = async_kw,
            name = test.name,
            source = test
                .source_file
                .split('/')
                .last()
                .unwrap_or(&test.source_file),
            line = test.line_number,
            body = py_body,
        )
    }

    /// Translate Rust code block to Python
    fn translate_rust_to_python(&self, rust_code: &str) -> String {
        let mut code = rust_code.to_string();

        // Remove outer braces
        code = code.trim().to_string();
        if code.starts_with('{') && code.ends_with('}') {
            code = code[1..code.len() - 1].to_string();
        }

        // Apply translations
        code = self.translate_statements(&code);

        code
    }

    fn translate_statements(&self, code: &str) -> String {
        let mut lines: Vec<String> = Vec::new();

        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let translated = self.translate_line(trimmed);
            if !translated.is_empty() {
                // Handle method chaining - if line starts with '.', append to previous
                if translated.starts_with('.') && !lines.is_empty() {
                    let last = lines.pop().unwrap();
                    lines.push(format!("{}{}", last, translated));
                } else {
                    lines.push(translated);
                }
            }
        }

        lines.join("\n")
    }

    pub(super) fn translate_line(&self, line: &str) -> String {
        let mut result = line.to_string();

        // Remove trailing semicolons
        result = result.trim_end_matches(';').to_string();

        // let (a, b) = ... -> a, b = ... (tuple unpacking)
        if result.starts_with("let (") {
            if let Some(eq_pos) = result.find(" = ") {
                let pattern = &result[4..eq_pos]; // Skip "let "
                let value = &result[eq_pos + 3..];
                // Remove outer parens from pattern if present
                let pattern = pattern.trim();
                let pattern = if pattern.starts_with('(') && pattern.ends_with(')') {
                    &pattern[1..pattern.len() - 1]
                } else {
                    pattern
                };
                result = format!("{} = {}", pattern, value);
            }
        }
        // let x = ... -> x = ...
        else if result.starts_with("let ") {
            result = result.replacen("let ", "", 1);
            // Remove mut
            result = result.replacen("mut ", "", 1);
        }

        // .unwrap() -> (remove)
        result = result.replace(".unwrap()", "");

        // .expect("...") -> (remove)
        if let Some(idx) = result.find(".expect(") {
            if let Some(end_idx) = result[idx..].find(')') {
                result = format!("{}{}", &result[..idx], &result[idx + end_idx + 1..]);
            }
        }

        // vec!["a", "b"] -> ["a", "b"]
        result = self.translate_vec_macro(&result);

        // "string".to_string() -> "string"
        result = result.replace(".to_string()", "");

        // assert_eq!(a, b) -> assert a == b
        result = self.translate_assert_eq(&result);

        // assert!(x) -> assert x
        result = self.translate_assert(&result);

        // Operator::Eq -> "="
        result = self.translate_operators(&result);

        // ExtractedValue::Int(42) -> 42
        result = self.translate_extracted_value(&result);

        // OrderDirection::Desc -> "desc" or OrderDirection enum
        result = self.translate_order_direction(&result);

        // Type::new(...) -> Type(...)
        result = self.translate_constructor(&result);

        // x.len() -> len(x)
        result = self.translate_len_method(&result);

        // Apply custom type mappings
        for (rust_type, py_type) in &self.config.type_mapping {
            result = result.replace(rust_type, py_type);
        }

        result.trim().to_string()
    }

    fn translate_len_method(&self, code: &str) -> String {
        let mut result = code.to_string();

        // Simple pattern: identifier.len() -> len(identifier)
        while let Some(len_pos) = result.find(".len()") {
            // Find the start of the identifier
            let before = &result[..len_pos];
            let mut start = len_pos;
            for (i, c) in before.chars().rev().enumerate() {
                if c.is_alphanumeric() || c == '_' {
                    start = len_pos - i - 1;
                } else {
                    break;
                }
            }
            let ident = &result[start..len_pos];
            let after = &result[len_pos + 6..];
            result = format!("{}len({}){}", &result[..start], ident, after);
        }

        result
    }

    pub(super) fn translate_vec_macro(&self, code: &str) -> String {
        let mut result = code.to_string();

        // Simple vec![] replacement
        while let Some(start) = result.find("vec![") {
            let rest = &result[start + 5..];
            if let Some(end) = self.find_matching_bracket(rest, '[', ']') {
                let inner = &rest[..end];
                let replacement = format!("[{}]", inner);
                result = format!("{}{}{}", &result[..start], replacement, &rest[end + 1..]);
            } else {
                break;
            }
        }

        result
    }

    fn find_matching_bracket(&self, s: &str, open: char, close: char) -> Option<usize> {
        let mut depth = 1;
        for (i, c) in s.char_indices() {
            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
        None
    }

    pub(super) fn translate_assert_eq(&self, code: &str) -> String {
        let mut result = code.to_string();

        // assert_eq!(a, b) -> assert a == b
        while let Some(start) = result.find("assert_eq!(") {
            let rest = &result[start + 11..];
            if let Some(end) = self.find_matching_bracket(rest, '(', ')') {
                let inner = &rest[..end];
                // Split on first top-level comma
                if let Some((left, right)) = self.split_on_comma(inner) {
                    let replacement = format!("assert {} == {}", left.trim(), right.trim());
                    result = format!("{}{}{}", &result[..start], replacement, &rest[end + 1..]);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        result
    }

    pub(super) fn translate_assert(&self, code: &str) -> String {
        let mut result = code.to_string();

        // assert!(x) -> assert x (but not assert_eq which we already handled)
        if result.contains("assert!(") && !result.contains("assert_eq") {
            while let Some(start) = result.find("assert!(") {
                let rest = &result[start + 8..];
                if let Some(end) = self.find_matching_bracket(rest, '(', ')') {
                    let inner = &rest[..end];
                    let replacement = format!("assert {}", inner.trim());
                    result = format!("{}{}{}", &result[..start], replacement, &rest[end + 1..]);
                } else {
                    break;
                }
            }
        }

        result
    }

    fn split_on_comma(&self, s: &str) -> Option<(String, String)> {
        let mut depth = 0;
        for (i, c) in s.char_indices() {
            match c {
                '(' | '[' | '{' | '<' => depth += 1,
                ')' | ']' | '}' | '>' => depth -= 1,
                ',' if depth == 0 => {
                    return Some((s[..i].to_string(), s[i + 1..].to_string()));
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn translate_operators(&self, code: &str) -> String {
        code.replace("Operator::Eq", "\"=\"")
            .replace("Operator::Ne", "\"!=\"")
            .replace("Operator::Gt", "\">\"")
            .replace("Operator::Gte", "\">=\"")
            .replace("Operator::Lt", "\"<\"")
            .replace("Operator::Lte", "\"<=\"")
            .replace("Operator::Like", "\"LIKE\"")
            .replace("Operator::In", "\"IN\"")
    }

    pub(super) fn translate_extracted_value(&self, code: &str) -> String {
        let mut result = code.to_string();

        // ExtractedValue::Int(42) -> 42
        result = self.replace_enum_variant(&result, "ExtractedValue::Int");
        result = self.replace_enum_variant(&result, "ExtractedValue::Float");
        result = self.replace_enum_variant(&result, "ExtractedValue::Bool");
        result = self.replace_enum_variant(&result, "ExtractedValue::String");

        result
    }

    fn translate_order_direction(&self, code: &str) -> String {
        code.replace("OrderDirection::Asc", "\"asc\"")
            .replace("OrderDirection::Desc", "\"desc\"")
    }

    fn replace_enum_variant(&self, code: &str, pattern: &str) -> String {
        let mut result = code.to_string();
        let search = format!("{}(", pattern);

        while let Some(start) = result.find(&search) {
            let rest = &result[start + search.len()..];
            if let Some(end) = self.find_matching_bracket(rest, '(', ')') {
                let inner = &rest[..end];
                result = format!("{}{}{}", &result[..start], inner, &rest[end + 1..]);
            } else {
                break;
            }
        }

        result
    }

    fn translate_constructor(&self, code: &str) -> String {
        let mut result = code.to_string();

        // QueryBuilder::new("table") -> QueryBuilder("table")
        // But only for types we know about
        let constructors = ["QueryBuilder::new", "WindowSpec::new"];
        for ctor in constructors {
            result = result.replace(ctor, &ctor.replace("::new", ""));
        }

        result
    }

    /// Generate a complete Python test file from extracted tests
    pub fn generate_test_file(&self, tests: &[RustTest]) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!(
            r#"""Auto-generated Python tests from Rust tests.

DO NOT EDIT - Regenerate with `cclab lens gen-python-test`
"""
import pytest
from {} import *

"#,
            self.config.python_module
        ));

        // Generate test functions
        for test in tests {
            output.push_str(&self.translate_to_python(test));
            output.push('\n');
        }

        output
    }
}
