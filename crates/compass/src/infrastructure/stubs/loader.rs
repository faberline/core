mod files;

use std::collections::HashMap;
use std::path::PathBuf;

use tree_sitter::Node;

use crate::domain::modules::import::ModuleInfo;
use crate::type_inference::annotation::parse_type_annotation;
use crate::type_inference::{Param, ParamKind, Type};

/// Stub file loader and cache
#[derive(Debug, Default)]
pub struct StubLoader {
    /// Loaded stubs (module path -> module info)
    stubs: HashMap<String, ModuleInfo>,
    /// Stub search paths (e.g., typeshed location)
    stub_paths: Vec<PathBuf>,
    /// Built-in stubs (preloaded)
    builtins_loaded: bool,
}

impl StubLoader {
    /// Parse stub definitions from AST
    fn parse_stub_definitions(&self, source: &str, node: &Node, info: &mut ModuleInfo) {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some((name, ty)) = self.parse_function_stub(source, &child) {
                        // Check for @overload decorator
                        if self.has_overload_decorator(source, &child) {
                            // Add to overload signatures
                            if let Some(existing) = info.exports.get_mut(&name) {
                                if let Type::Overloaded { signatures } = existing {
                                    signatures.push(ty);
                                } else {
                                    // Convert to overloaded
                                    let old = existing.clone();
                                    *existing = Type::Overloaded {
                                        signatures: vec![old, ty],
                                    };
                                }
                            } else {
                                info.exports.insert(name, ty);
                            }
                        } else {
                            info.exports.insert(name, ty);
                        }
                    }
                }
                "class_definition" => {
                    if let Some((name, ty)) = self.parse_class_stub(source, &child) {
                        info.exports.insert(name, ty);
                    }
                }
                "expression_statement" => {
                    // Type alias: Name = Type or Name: TypeAlias = Type
                    if let Some((name, ty)) = self.parse_type_alias(source, &child) {
                        info.exports.insert(name, ty);
                    }
                }
                "import_from_statement" | "import_statement" => {
                    // Track re-exports for __init__.pyi files
                    self.parse_import_export(source, &child, info);
                }
                _ => {}
            }
        }
    }

    /// Check if function has @overload decorator
    fn has_overload_decorator(&self, source: &str, node: &Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "decorator" {
                let text = self.node_text(source, &child);
                if text.contains("overload") {
                    return true;
                }
            }
        }
        false
    }

    /// Parse a function stub
    fn parse_function_stub(&self, source: &str, node: &Node) -> Option<(String, Type)> {
        let name_node = node.child_by_field_name("name")?;
        let name = self.node_text(source, &name_node).to_string();

        let params_node = node.child_by_field_name("parameters")?;
        let params = self.parse_parameters(source, &params_node);

        let return_type = node
            .child_by_field_name("return_type")
            .map(|n| parse_type_annotation(source, &n))
            .unwrap_or(Type::Any);

        Some((
            name,
            Type::Callable {
                params,
                ret: Box::new(return_type),
            },
        ))
    }

    /// Parse function parameters
    fn parse_parameters(&self, source: &str, node: &Node) -> Vec<Param> {
        let mut params = Vec::new();
        let mut cursor = node.walk();
        let mut positional_only = false;
        let mut keyword_only = false;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    // Simple parameter without annotation
                    let name = self.node_text(source, &child).to_string();
                    if name != "self" && name != "cls" {
                        let kind = if keyword_only {
                            ParamKind::KeywordOnly
                        } else if positional_only {
                            ParamKind::PositionalOnly
                        } else {
                            ParamKind::Positional
                        };
                        params.push(Param {
                            name,
                            ty: Type::Any,
                            has_default: false,
                            kind,
                        });
                    }
                }
                "typed_parameter" | "typed_default_parameter" => {
                    if let Some(param) =
                        self.parse_typed_param(source, &child, keyword_only, positional_only)
                    {
                        params.push(param);
                    }
                }
                "default_parameter" => {
                    if let Some(param) =
                        self.parse_default_param(source, &child, keyword_only, positional_only)
                    {
                        params.push(param);
                    }
                }
                "list_splat_pattern" => {
                    // *args
                    if let Some(name_node) = child.child(1) {
                        let name = self.node_text(source, &name_node).to_string();
                        params.push(Param {
                            name,
                            ty: Type::Any,
                            has_default: false,
                            kind: ParamKind::VarPositional,
                        });
                    }
                    keyword_only = true;
                }
                "dictionary_splat_pattern" => {
                    // **kwargs
                    if let Some(name_node) = child.child(1) {
                        let name = self.node_text(source, &name_node).to_string();
                        params.push(Param {
                            name,
                            ty: Type::Any,
                            has_default: false,
                            kind: ParamKind::VarKeyword,
                        });
                    }
                }
                "/" => {
                    positional_only = true;
                }
                "*" => {
                    keyword_only = true;
                }
                _ => {}
            }
        }

        params
    }

    /// Parse a typed parameter
    fn parse_typed_param(
        &self,
        source: &str,
        node: &Node,
        keyword_only: bool,
        positional_only: bool,
    ) -> Option<Param> {
        let name_node = node.child_by_field_name("name")?;
        let name = self.node_text(source, &name_node).to_string();

        if name == "self" || name == "cls" {
            return None;
        }

        let ty = node
            .child_by_field_name("type")
            .map(|n| parse_type_annotation(source, &n))
            .unwrap_or(Type::Any);

        let has_default = node.child_by_field_name("value").is_some();

        let kind = if keyword_only {
            ParamKind::KeywordOnly
        } else if positional_only {
            ParamKind::PositionalOnly
        } else {
            ParamKind::Positional
        };

        Some(Param {
            name,
            ty,
            has_default,
            kind,
        })
    }

    /// Parse a default parameter (no type annotation)
    fn parse_default_param(
        &self,
        source: &str,
        node: &Node,
        keyword_only: bool,
        positional_only: bool,
    ) -> Option<Param> {
        let name_node = node.child_by_field_name("name")?;
        let name = self.node_text(source, &name_node).to_string();

        if name == "self" || name == "cls" {
            return None;
        }

        let kind = if keyword_only {
            ParamKind::KeywordOnly
        } else if positional_only {
            ParamKind::PositionalOnly
        } else {
            ParamKind::Positional
        };

        Some(Param {
            name,
            ty: Type::Any,
            has_default: true,
            kind,
        })
    }

    /// Parse a class stub
    fn parse_class_stub(&self, source: &str, node: &Node) -> Option<(String, Type)> {
        let name_node = node.child_by_field_name("name")?;
        let name = self.node_text(source, &name_node).to_string();

        // For now, return a ClassType
        // In the future, we could parse methods and attributes
        Some((name.clone(), Type::ClassType { name, module: None }))
    }

    /// Parse a type alias
    fn parse_type_alias(&self, source: &str, node: &Node) -> Option<(String, Type)> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "assignment" {
                let left = child.child_by_field_name("left")?;
                let right = child.child_by_field_name("right")?;

                if left.kind() == "identifier" {
                    let name = self.node_text(source, &left).to_string();
                    let ty = parse_type_annotation(source, &right);
                    return Some((name, ty));
                }
            }
        }
        None
    }

    /// Parse import/export for re-exports
    fn parse_import_export(&self, source: &str, node: &Node, info: &mut ModuleInfo) {
        if node.kind() == "import_from_statement" {
            // from module import name -> re-export
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "dotted_name" || child.kind() == "identifier" {
                    let text = self.node_text(source, &child);
                    // Skip the module name, just track imported names
                    if child.prev_sibling().map(|n| n.kind()) == Some("import") {
                        info.exports.insert(
                            text.to_string(),
                            Type::Instance {
                                name: text.to_string(),
                                module: None,
                                type_args: vec![],
                            },
                        );
                    }
                }
                if child.kind() == "aliased_import" {
                    if let Some(alias) = child.child_by_field_name("alias") {
                        let name = self.node_text(source, &alias);
                        info.exports.insert(
                            name.to_string(),
                            Type::Instance {
                                name: name.to_string(),
                                module: None,
                                type_args: vec![],
                            },
                        );
                    }
                }
            }
        }
    }

    /// Get text of a node
    fn node_text<'a>(&self, source: &'a str, node: &Node) -> &'a str {
        node.utf8_text(source.as_bytes()).unwrap_or("")
    }
}

#[cfg(test)]
mod tests;
