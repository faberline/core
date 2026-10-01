//! Rust symbol collection from AST
//!
//! This module provides functionality for extracting Rust symbols
//! (structs, enums, traits, impl blocks, functions, etc.) from tree-sitter AST.

use tree_sitter::Node;

use crate::domain::rust_type_system::types::{
    EnumDef, ImplBlock, RustParam, RustType, RustTypeParam, StructDef, TraitDef, TraitId,
    Visibility, WherePredicate,
};

mod bounds;
mod fields;
mod helpers;
mod signature;

// ============================================================================
// Symbol Types
// ============================================================================

/// Collected symbols from a Rust file
#[derive(Debug, Clone, Default)]
pub struct RustSymbols {
    /// Struct definitions
    pub structs: Vec<StructDef>,
    /// Enum definitions
    pub enums: Vec<EnumDef>,
    /// Trait definitions
    pub traits: Vec<TraitDef>,
    /// Impl blocks
    pub impls: Vec<ImplBlock>,
    /// Function definitions
    pub functions: Vec<RustFunction>,
    /// Constants
    pub constants: Vec<RustConstant>,
    /// Type aliases
    pub type_aliases: Vec<RustTypeAlias>,
}

/// Rust function definition
#[derive(Debug, Clone)]
pub struct RustFunction {
    /// Function name
    pub name: String,
    /// Visibility
    pub visibility: Visibility,
    /// Generic parameters
    pub type_params: Vec<RustTypeParam>,
    /// Parameters
    pub params: Vec<RustParam>,
    /// Return type
    pub return_type: RustType,
    /// Where clause
    pub where_bounds: Vec<WherePredicate>,
    /// Is async
    pub is_async: bool,
    /// Is unsafe
    pub is_unsafe: bool,
    /// Is const
    pub is_const: bool,
    /// Source span (start, end)
    pub span: (usize, usize),
}

/// Rust constant definition
#[derive(Debug, Clone)]
pub struct RustConstant {
    /// Constant name
    pub name: String,
    /// Visibility
    pub visibility: Visibility,
    /// Type
    pub ty: RustType,
    /// Source span
    pub span: (usize, usize),
}

/// Rust type alias
#[derive(Debug, Clone)]
pub struct RustTypeAlias {
    /// Alias name
    pub name: String,
    /// Visibility
    pub visibility: Visibility,
    /// Generic parameters
    pub type_params: Vec<RustTypeParam>,
    /// Target type
    pub target: RustType,
    /// Source span
    pub span: (usize, usize),
}

// ============================================================================
// Symbol Collector
// ============================================================================

/// Collects Rust symbols from AST
pub struct RustSymbolCollector {
    /// Collected symbols
    symbols: RustSymbols,
    /// Trait ID counter
    trait_id_counter: usize,
    /// Type var counter
    type_var_counter: usize,
}

impl RustSymbolCollector {
    /// Create a new symbol collector
    pub fn new() -> Self {
        Self {
            symbols: RustSymbols::default(),
            trait_id_counter: 0,
            type_var_counter: 0,
        }
    }

    /// Collect symbols from a tree-sitter node (source_file)
    pub fn collect(&mut self, node: &Node, source: &str) -> RustSymbols {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            self.collect_item(&child, source);
        }

        std::mem::take(&mut self.symbols)
    }

    /// Collect a single item
    fn collect_item(&mut self, node: &Node, source: &str) {
        match node.kind() {
            "struct_item" => self.collect_struct(node, source),
            "enum_item" => self.collect_enum(node, source),
            "trait_item" => self.collect_trait(node, source),
            "impl_item" => self.collect_impl(node, source),
            "function_item" => self.collect_function(node, source),
            "const_item" => self.collect_const(node, source),
            "type_item" => self.collect_type_alias(node, source),
            "mod_item" => self.collect_mod(node, source),
            _ => {}
        }
    }

    /// Collect struct definition
    fn collect_struct(&mut self, node: &Node, source: &str) {
        let name = self.get_name(node, source);
        let visibility = self.get_visibility(node);
        let type_params = self.collect_type_params(node, source);
        let fields = self.collect_struct_fields(node, source);
        let where_bounds = self.collect_where_clause(node, source);

        self.symbols.structs.push(StructDef {
            name,
            module: None,
            type_params,
            fields,
            where_bounds,
            visibility,
        });
    }

    /// Collect enum definition
    fn collect_enum(&mut self, node: &Node, source: &str) {
        let name = self.get_name(node, source);
        let visibility = self.get_visibility(node);
        let type_params = self.collect_type_params(node, source);
        let variants = self.collect_enum_variants(node, source);
        let where_bounds = self.collect_where_clause(node, source);

        self.symbols.enums.push(EnumDef {
            name,
            module: None,
            type_params,
            variants,
            where_bounds,
            visibility,
        });
    }

    /// Collect trait definition
    fn collect_trait(&mut self, node: &Node, source: &str) {
        let name = self.get_name(node, source);
        let id = TraitId(self.trait_id_counter);
        self.trait_id_counter += 1;

        let type_params = self.collect_type_params(node, source);
        let supertraits = self.collect_supertraits(node, source);

        self.symbols.traits.push(TraitDef {
            id,
            name,
            module: None,
            type_params,
            supertraits,
            associated_types: vec![],
            required_methods: vec![],
            provided_methods: vec![],
            is_auto: false,
            is_marker: false,
        });
    }

    /// Collect impl block
    fn collect_impl(&mut self, node: &Node, source: &str) {
        let type_params = self.collect_type_params(node, source);
        let trait_ref = self.collect_impl_trait(node, source);
        let self_type = self.collect_impl_type(node, source);
        let where_bounds = self.collect_where_clause(node, source);
        let methods = self.collect_impl_methods(node, source);

        self.symbols.impls.push(ImplBlock {
            type_params,
            trait_ref,
            self_type,
            where_bounds,
            methods,
            associated_types: vec![],
            associated_consts: vec![],
            is_negative: false,
            is_unsafe: node.child_by_field_name("unsafe").is_some(),
        });
    }

    /// Collect function definition
    fn collect_function(&mut self, node: &Node, source: &str) {
        let name = self.get_name(node, source);
        let visibility = self.get_visibility(node);
        let type_params = self.collect_type_params(node, source);
        let params = self.collect_function_params(node, source);
        let return_type = self.collect_return_type(node, source);
        let where_bounds = self.collect_where_clause(node, source);

        self.symbols.functions.push(RustFunction {
            name,
            visibility,
            type_params,
            params,
            return_type,
            where_bounds,
            is_async: node.child_by_field_name("async").is_some(),
            is_unsafe: node.child_by_field_name("unsafe").is_some(),
            is_const: node.child_by_field_name("const").is_some(),
            span: (node.start_byte(), node.end_byte()),
        });
    }

    /// Collect const definition
    fn collect_const(&mut self, node: &Node, source: &str) {
        let name = self.get_name(node, source);
        let visibility = self.get_visibility(node);
        let ty = if let Some(type_node) = node.child_by_field_name("type") {
            self.parse_type(&type_node, source)
        } else {
            RustType::Infer
        };

        self.symbols.constants.push(RustConstant {
            name,
            visibility,
            ty,
            span: (node.start_byte(), node.end_byte()),
        });
    }

    /// Collect type alias
    fn collect_type_alias(&mut self, node: &Node, source: &str) {
        let name = self.get_name(node, source);
        let visibility = self.get_visibility(node);
        let type_params = self.collect_type_params(node, source);
        let target = if let Some(type_node) = node.child_by_field_name("type") {
            self.parse_type(&type_node, source)
        } else {
            RustType::Infer
        };

        self.symbols.type_aliases.push(RustTypeAlias {
            name,
            visibility,
            type_params,
            target,
            span: (node.start_byte(), node.end_byte()),
        });
    }

    /// Collect items from a module
    fn collect_mod(&mut self, node: &Node, source: &str) {
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                self.collect_item(&child, source);
            }
        }
    }
}

impl Default for RustSymbolCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
