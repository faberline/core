//! Rust public export scanner for Python-oriented code generation.
//!
//! Parses Rust source files using tree-sitter to extract public items:
//! - `pub struct` (data or stateful)
//! - `pub enum`
//! - `pub fn` / `pub async fn`
//!
//! Convention: public Rust items can be discovered and projected into Python-facing generators.

use crate::domain::rust_source_scan::exports::{RustExports, RustMethod, StructKind};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Node, Parser};

mod items;
mod signature;

/// Scanner for Rust public exports
pub struct RustScanner {
    parser: Parser,
}

impl RustScanner {
    /// Create a new scanner
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .context("Failed to set Rust language for parser")?;
        Ok(Self { parser })
    }

    /// Scan a Rust crate directory for public exports
    pub fn scan_crate(&mut self, crate_path: &Path) -> Result<RustExports> {
        let src_dir = if crate_path.join("src").exists() {
            crate_path.join("src")
        } else {
            crate_path.to_path_buf()
        };

        let lib_rs = src_dir.join("lib.rs");
        if !lib_rs.exists() {
            anyhow::bail!("No lib.rs found in {}", src_dir.display());
        }

        let mut exports = RustExports::default();
        let mut impl_methods: HashMap<String, Vec<RustMethod>> = HashMap::new();

        // First pass: scan lib.rs for pub items and pub mod declarations
        let content = std::fs::read_to_string(&lib_rs)
            .with_context(|| format!("Failed to read {}", lib_rs.display()))?;

        let (lib_exports, lib_impls) = self.scan_file(&content)?;
        for (name, methods) in lib_impls {
            impl_methods.entry(name).or_default().extend(methods);
        }
        exports.merge(lib_exports);

        // Find and scan pub mod files
        let pub_mods = self.find_pub_mods(&content)?;
        for mod_name in pub_mods {
            let mod_file = src_dir.join(format!("{}.rs", mod_name));
            let mod_dir = src_dir.join(&mod_name).join("mod.rs");

            let mod_path = if mod_file.exists() {
                mod_file
            } else if mod_dir.exists() {
                mod_dir
            } else {
                continue;
            };

            if let Ok(mod_content) = std::fs::read_to_string(&mod_path) {
                let (mod_exports, mod_impls) = self.scan_file(&mod_content)?;
                for (name, methods) in mod_impls {
                    impl_methods.entry(name).or_default().extend(methods);
                }
                exports.merge(mod_exports);
            }
        }

        // Attach methods to structs
        for s in &mut exports.structs {
            if let Some(methods) = impl_methods.remove(&s.name) {
                s.methods = methods;
                // If has methods that take &self or &mut self, mark as stateful
                if s.methods.iter().any(|m| m.takes_self || m.takes_mut_self) {
                    s.kind = StructKind::Stateful;
                }
            }
        }

        Ok(exports)
    }

    /// Scan a single file for public items
    pub fn scan_file(
        &mut self,
        content: &str,
    ) -> Result<(RustExports, HashMap<String, Vec<RustMethod>>)> {
        let tree = self
            .parser
            .parse(content, None)
            .context("Failed to parse Rust source")?;

        let root = tree.root_node();
        let mut exports = RustExports::default();
        let mut impl_methods: HashMap<String, Vec<RustMethod>> = HashMap::new();

        self.visit_node(root, content, &mut exports, &mut impl_methods)?;

        Ok((exports, impl_methods))
    }

    /// Find `pub mod` declarations in a file
    fn find_pub_mods(&mut self, content: &str) -> Result<Vec<String>> {
        let tree = self
            .parser
            .parse(content, None)
            .context("Failed to parse Rust source")?;

        let root = tree.root_node();
        let mut mods = Vec::new();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "mod_item" {
                if self.has_pub_visibility(child, content) {
                    if let Some(name) = self.get_child_text(child, "identifier", content) {
                        // Only include mods without body (external files)
                        if child.child_by_field_name("body").is_none() {
                            mods.push(name);
                        }
                    }
                }
            }
        }

        Ok(mods)
    }

    /// Visit AST node and extract public items
    fn visit_node(
        &self,
        node: Node,
        source: &str,
        exports: &mut RustExports,
        impl_methods: &mut HashMap<String, Vec<RustMethod>>,
    ) -> Result<()> {
        match node.kind() {
            "struct_item" => {
                if self.has_pub_visibility(node, source) {
                    if let Some(s) = self.extract_struct(node, source)? {
                        exports.structs.push(s);
                    }
                }
            }
            "enum_item" => {
                if self.has_pub_visibility(node, source) {
                    if let Some(e) = self.extract_enum(node, source)? {
                        exports.enums.push(e);
                    }
                }
            }
            "function_item" => {
                if self.has_pub_visibility(node, source) {
                    if let Some(f) = self.extract_function(node, source)? {
                        exports.functions.push(f);
                    }
                }
            }
            "impl_item" => {
                // Extract methods from impl blocks
                if let Some((type_name, methods)) = self.extract_impl_methods(node, source)? {
                    impl_methods.entry(type_name).or_default().extend(methods);
                }
            }
            _ => {}
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node(child, source, exports, impl_methods)?;
        }

        Ok(())
    }

    /// Check if a node has `pub` visibility
    fn has_pub_visibility(&self, node: Node, source: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                let text = self.node_text(child, source);
                return text.starts_with("pub");
            }
        }
        false
    }

    /// Get text from a child node by kind
    fn get_child_text(&self, node: Node, kind: &str, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == kind {
                return Some(self.node_text(child, source));
            }
        }
        None
    }

    /// Get text from a node
    fn node_text(&self, node: Node, source: &str) -> String {
        source[node.byte_range()].to_string()
    }
}

impl Default for RustScanner {
    fn default() -> Self {
        Self::new().expect("Failed to create RustScanner")
    }
}

#[cfg(test)]
mod tests;
