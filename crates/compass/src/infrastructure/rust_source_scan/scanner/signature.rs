//! Parameters, return types, derives and doc comments.

use super::RustScanner;
use crate::domain::rust_source_scan::exports::RustParam;
use anyhow::Result;
use tree_sitter::Node;

impl RustScanner {
    /// Check self parameter type
    pub(super) fn check_self_param(&self, node: Node, source: &str) -> (bool, bool, bool) {
        if let Some(params) = node.child_by_field_name("parameters") {
            let mut cursor = params.walk();
            for child in params.children(&mut cursor) {
                if child.kind() == "self_parameter" {
                    let text = self.node_text(child, source);
                    if text.contains("&mut self") {
                        return (false, true, false);
                    } else if text.contains("&self") || text == "self" {
                        return (true, false, false);
                    }
                }
            }
        }
        (false, false, true)
    }

    /// Extract function parameters
    pub(super) fn extract_parameters(&self, node: Node, source: &str) -> Result<Vec<RustParam>> {
        let mut params = Vec::new();

        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                if child.kind() == "parameter" {
                    let name = child
                        .child_by_field_name("pattern")
                        .map(|n| self.node_text(n, source))
                        .unwrap_or_default();

                    let ty = child
                        .child_by_field_name("type")
                        .map(|n| self.node_text(n, source))
                        .unwrap_or_default();

                    // Skip Python-oriented internal params
                    if name == "py" || name == "_py" || ty.contains("Python") {
                        continue;
                    }

                    let is_optional = ty.starts_with("Option<");

                    params.push(RustParam {
                        name,
                        ty,
                        is_optional,
                        default_value: None,
                    });
                }
            }
        }

        Ok(params)
    }

    /// Extract return type
    pub(super) fn extract_return_type(&self, node: Node, source: &str) -> Option<String> {
        node.child_by_field_name("return_type")
            .map(|n| self.node_text(n, source))
    }

    /// Extract #[derive(...)] attributes
    pub(super) fn extract_derives(&self, node: Node, source: &str) -> Vec<String> {
        let mut derives = Vec::new();

        // Look at preceding siblings for attributes
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "attribute_item" {
                let text = self.node_text(sibling, source);
                if text.contains("derive") {
                    // Extract derive contents: #[derive(Clone, Debug)] -> ["Clone", "Debug"]
                    if let Some(start) = text.find("derive(") {
                        let rest = &text[start + 7..];
                        if let Some(end) = rest.find(')') {
                            let inner = &rest[..end];
                            for item in inner.split(',') {
                                derives.push(item.trim().to_string());
                            }
                        }
                    }
                }
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }

        derives
    }

    /// Extract docstring from preceding comments
    pub(super) fn extract_docstring(&self, node: Node, source: &str) -> Option<String> {
        let mut doc_lines = Vec::new();

        let start_byte = node.start_byte();
        let preceding = &source[..start_byte];

        for line in preceding.lines().rev() {
            let trimmed = line.trim();
            if trimmed.starts_with("///") {
                let doc = trimmed.trim_start_matches("///").trim();
                doc_lines.insert(0, doc.to_string());
            } else if trimmed.starts_with("#[doc") {
                // Handle #[doc = "..."]
                if let Some(start) = trimmed.find('"') {
                    if let Some(end) = trimmed.rfind('"') {
                        if start < end {
                            let doc = &trimmed[start + 1..end];
                            doc_lines.insert(0, doc.to_string());
                        }
                    }
                }
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                break;
            }
        }

        if doc_lines.is_empty() {
            None
        } else {
            Some(doc_lines.join("\n"))
        }
    }
}
