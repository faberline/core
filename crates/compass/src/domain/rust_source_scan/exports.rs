//! Model types for scanned Rust public exports.

/// Scanned Rust exports from a crate
#[derive(Debug, Default)]
pub struct RustExports {
    /// Public structs
    pub structs: Vec<RustStruct>,
    /// Public enums
    pub enums: Vec<RustEnum>,
    /// Public functions
    pub functions: Vec<RustFunction>,
}

impl RustExports {
    /// Merge another RustExports into this one
    pub fn merge(&mut self, other: RustExports) {
        self.structs.extend(other.structs);
        self.enums.extend(other.enums);
        self.functions.extend(other.functions);
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.structs.is_empty() && self.enums.is_empty() && self.functions.is_empty()
    }
}

/// Kind of struct (determines wrapper strategy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructKind {
    /// Data struct: all fields simple types, no methods.
    Data,
    /// Stateful struct: has methods taking `&self` or `&mut self` → `Arc<RwLock<T>>`
    Stateful,
    /// Has generic type parameters → mark as TODO
    Generic,
}

/// Public struct with its fields and methods
#[derive(Debug)]
pub struct RustStruct {
    pub name: String,
    pub kind: StructKind,
    pub fields: Vec<RustField>,
    pub methods: Vec<RustMethod>,
    pub docstring: Option<String>,
    pub derives: Vec<String>,
    /// Whether struct derives Clone
    pub has_clone: bool,
    /// Whether struct derives Default
    pub has_default: bool,
}

/// Struct field
#[derive(Debug)]
pub struct RustField {
    pub name: String,
    pub ty: String,
    pub is_public: bool,
    pub docstring: Option<String>,
}

/// Struct method (from impl block)
#[derive(Debug, Clone)]
pub struct RustMethod {
    pub name: String,
    pub params: Vec<RustParam>,
    pub return_type: Option<String>,
    pub is_async: bool,
    pub is_static: bool,
    pub takes_self: bool,
    pub takes_mut_self: bool,
    pub docstring: Option<String>,
}

/// Function parameter
#[derive(Debug, Clone)]
pub struct RustParam {
    pub name: String,
    pub ty: String,
    pub is_optional: bool,
    pub default_value: Option<String>,
}

/// Public enum
#[derive(Debug)]
pub struct RustEnum {
    pub name: String,
    pub variants: Vec<RustEnumVariant>,
    pub docstring: Option<String>,
    /// Whether all variants are unit variants (no data)
    pub is_simple: bool,
}

/// Enum variant
#[derive(Debug)]
pub struct RustEnumVariant {
    pub name: String,
    /// None for unit variants, Some for tuple/struct variants
    pub data: Option<String>,
    pub docstring: Option<String>,
}

/// Public function
#[derive(Debug)]
pub struct RustFunction {
    pub name: String,
    pub params: Vec<RustParam>,
    pub return_type: Option<String>,
    pub is_async: bool,
    pub docstring: Option<String>,
}
