//! Struct, enum, function and impl-method extraction.

use super::RustScanner;
use crate::domain::rust_source_scan::exports::{
    RustEnum, RustEnumVariant, RustField, RustFunction, RustMethod, RustStruct, StructKind,
};
use anyhow::Result;
use tree_sitter::Node;

impl RustScanner {
    /// Extract struct definition
    pub(super) fn extract_struct(&self, node: Node, source: &str) -> Result<Option<RustStruct>> {
        let name = match self.get_child_text(node, "type_identifier", source) {
            Some(n) => n,
            None => return Ok(None),
        };

        // Check for generic parameters
        let has_generics = node
            .children(&mut node.walk())
            .any(|c| c.kind() == "type_parameters");

        let kind = if has_generics {
            StructKind::Generic
        } else {
            StructKind::Data // Will be updated if methods are found
        };

        let docstring = self.extract_docstring(node, source);
        let derives = self.extract_derives(node, source);
        let has_clone = derives.iter().any(|d| d == "Clone");
        let has_default = derives.iter().any(|d| d == "Default");

        // Extract fields
        let mut fields = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "field_declaration" {
                    if let Some(field) = self.extract_field(child, source)? {
                        fields.push(field);
                    }
                }
            }
        }

        Ok(Some(RustStruct {
            name,
            kind,
            fields,
            methods: Vec::new(), // Filled in later
            docstring,
            derives,
            has_clone,
            has_default,
        }))
    }

    /// Extract struct field
    fn extract_field(&self, node: Node, source: &str) -> Result<Option<RustField>> {
        let name = match self.get_child_text(node, "field_identifier", source) {
            Some(n) => n,
            None => return Ok(None),
        };

        let ty = node
            .child_by_field_name("type")
            .map(|n| self.node_text(n, source))
            .unwrap_or_default();

        let is_public = self.has_pub_visibility(node, source);
        let docstring = self.extract_docstring(node, source);

        Ok(Some(RustField {
            name,
            ty,
            is_public,
            docstring,
        }))
    }

    /// Extract enum definition
    pub(super) fn extract_enum(&self, node: Node, source: &str) -> Result<Option<RustEnum>> {
        let name = match self.get_child_text(node, "type_identifier", source) {
            Some(n) => n,
            None => return Ok(None),
        };

        let docstring = self.extract_docstring(node, source);

        // Extract variants
        let mut variants = Vec::new();
        let mut is_simple = true;

        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "enum_variant" {
                    if let Some(variant) = self.extract_enum_variant(child, source)? {
                        if variant.data.is_some() {
                            is_simple = false;
                        }
                        variants.push(variant);
                    }
                }
            }
        }

        Ok(Some(RustEnum {
            name,
            variants,
            docstring,
            is_simple,
        }))
    }

    /// Extract enum variant
    fn extract_enum_variant(&self, node: Node, source: &str) -> Result<Option<RustEnumVariant>> {
        let name = match self.get_child_text(node, "identifier", source) {
            Some(n) => n,
            None => return Ok(None),
        };

        let docstring = self.extract_docstring(node, source);

        // Check for tuple or struct variant data
        let data = node
            .children(&mut node.walk())
            .find(|c| {
                c.kind() == "ordered_field_declaration_list" || c.kind() == "field_declaration_list"
            })
            .map(|c| self.node_text(c, source));

        Ok(Some(RustEnumVariant {
            name,
            data,
            docstring,
        }))
    }

    /// Extract function definition
    pub(super) fn extract_function(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Option<RustFunction>> {
        let name = match self.get_child_text(node, "identifier", source) {
            Some(n) => n,
            None => return Ok(None),
        };

        let docstring = self.extract_docstring(node, source);
        let is_async = self.node_text(node, source).contains("async fn");

        let params = self.extract_parameters(node, source)?;
        let return_type = self.extract_return_type(node, source);

        Ok(Some(RustFunction {
            name,
            params,
            return_type,
            is_async,
            docstring,
        }))
    }

    /// Extract methods from impl block
    pub(super) fn extract_impl_methods(
        &self,
        node: Node,
        source: &str,
    ) -> Result<Option<(String, Vec<RustMethod>)>> {
        // Get the type being implemented
        let type_name = node
            .child_by_field_name("type")
            .map(|n| self.node_text(n, source))
            .unwrap_or_default();

        if type_name.is_empty() {
            return Ok(None);
        }

        // Skip trait implementations for now
        if node.children(&mut node.walk()).any(|c| c.kind() == "trait") {
            return Ok(None);
        }

        let mut methods = Vec::new();

        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "function_item" {
                    if self.has_pub_visibility(child, source) {
                        if let Some(method) = self.extract_method(child, source)? {
                            methods.push(method);
                        }
                    }
                }
            }
        }

        if methods.is_empty() {
            return Ok(None);
        }

        Ok(Some((type_name, methods)))
    }

    /// Extract method from function item
    fn extract_method(&self, node: Node, source: &str) -> Result<Option<RustMethod>> {
        let name = match self.get_child_text(node, "identifier", source) {
            Some(n) => n,
            None => return Ok(None),
        };

        let docstring = self.extract_docstring(node, source);
        let is_async = self.node_text(node, source).contains("async fn");

        // Check for self parameter
        let (takes_self, takes_mut_self, is_static) = self.check_self_param(node, source);

        let params = self.extract_parameters(node, source)?;
        let return_type = self.extract_return_type(node, source);

        Ok(Some(RustMethod {
            name,
            params,
            return_type,
            is_async,
            is_static,
            takes_self,
            takes_mut_self,
            docstring,
        }))
    }
}
