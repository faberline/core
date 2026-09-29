//! Extract Rust tests and convert to Python tests.
//!
//! This module extracts `#[test]` functions from Rust source files
//! and translates them to equivalent Python pytest tests.

use anyhow::{Context, Result};
use std::path::Path;

mod translate;

/// Configuration for test extraction
#[derive(Debug, Clone, Default)]
pub struct TestExtractorConfig {
    /// Rust struct name to Python class name mapping
    pub type_mapping: Vec<(String, String)>,
    /// Module import path (e.g., "cclab.titan")
    pub python_module: String,
}

/// Extracted Rust test
#[derive(Debug, Clone)]
pub struct RustTest {
    pub name: String,
    pub body: String,
    pub source_file: String,
    pub line_number: usize,
    pub is_async: bool,
}

/// Test extractor and translator
pub struct TestExtractor {
    config: TestExtractorConfig,
}

impl TestExtractor {
    pub fn new(config: TestExtractorConfig) -> Self {
        Self { config }
    }

    /// Extract tests from a Rust source file
    pub fn extract_tests(&self, path: &Path) -> Result<Vec<RustTest>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read: {}", path.display()))?;

        self.extract_tests_from_source(&content, path.to_string_lossy().to_string())
    }

    /// Extract tests from source string
    pub fn extract_tests_from_source(
        &self,
        content: &str,
        source_file: String,
    ) -> Result<Vec<RustTest>> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .context("Failed to set Rust language")?;

        let tree = parser
            .parse(content, None)
            .context("Failed to parse Rust source")?;

        let mut tests = Vec::new();
        self.visit_node(tree.root_node(), content, &source_file, &mut tests)?;

        Ok(tests)
    }

    fn visit_node(
        &self,
        node: tree_sitter::Node,
        source: &str,
        source_file: &str,
        tests: &mut Vec<RustTest>,
    ) -> Result<()> {
        if node.kind() == "function_item" {
            if let Some(test) = self.extract_test_function(node, source, source_file)? {
                tests.push(test);
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node(child, source, source_file, tests)?;
        }

        Ok(())
    }

    fn extract_test_function(
        &self,
        node: tree_sitter::Node,
        source: &str,
        source_file: &str,
    ) -> Result<Option<RustTest>> {
        // Check for #[test] attribute
        let has_test_attr = self.has_test_attribute(node, source);
        if !has_test_attr {
            return Ok(None);
        }

        // Get function name
        let name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(n, source))
            .unwrap_or_default();

        // Check if async
        let is_async = source[node.byte_range()].contains("async fn");

        // Get function body
        let body = node
            .child_by_field_name("body")
            .map(|n| self.node_text(n, source))
            .unwrap_or_default();

        let line_number = node.start_position().row + 1;

        Ok(Some(RustTest {
            name,
            body,
            source_file: source_file.to_string(),
            line_number,
            is_async,
        }))
    }

    fn has_test_attribute(&self, node: tree_sitter::Node, source: &str) -> bool {
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "attribute_item" {
                let text = self.node_text(sibling, source);
                if text.contains("#[test]") || text.contains("#[tokio::test]") {
                    return true;
                }
            } else if sibling.kind() != "attribute_item" && sibling.kind() != "line_comment" {
                break;
            }
            prev = sibling.prev_sibling();
        }
        false
    }

    fn node_text(&self, node: tree_sitter::Node, source: &str) -> String {
        source[node.byte_range()].to_string()
    }
}

#[cfg(test)]
mod tests;
