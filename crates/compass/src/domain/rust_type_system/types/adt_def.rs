use crate::domain::rust_type_system::types::{RustType, RustTypeParam, Visibility, WherePredicate};

// ============================================================================
// Rust Symbol Types
// ============================================================================

/// Rust struct definition
#[derive(Debug, Clone)]
pub struct StructDef {
    /// Struct name
    pub name: String,
    /// Module path
    pub module: Option<String>,
    /// Type parameters
    pub type_params: Vec<RustTypeParam>,
    /// Fields
    pub fields: StructFields,
    /// Where clause
    pub where_bounds: Vec<WherePredicate>,
    /// Visibility
    pub visibility: Visibility,
}

/// Struct field variants
#[derive(Debug, Clone)]
pub enum StructFields {
    /// Named fields (struct { field: Type })
    Named(Vec<StructField>),
    /// Tuple fields (struct (Type, Type))
    Tuple(Vec<RustType>),
    /// Unit struct (struct Name;)
    Unit,
}

/// Named struct field
#[derive(Debug, Clone)]
pub struct StructField {
    /// Field name
    pub name: String,
    /// Field type
    pub ty: RustType,
    /// Visibility
    pub visibility: Visibility,
}

/// Rust enum definition
#[derive(Debug, Clone)]
pub struct EnumDef {
    /// Enum name
    pub name: String,
    /// Module path
    pub module: Option<String>,
    /// Type parameters
    pub type_params: Vec<RustTypeParam>,
    /// Variants
    pub variants: Vec<EnumVariant>,
    /// Where clause
    pub where_bounds: Vec<WherePredicate>,
    /// Visibility
    pub visibility: Visibility,
}

/// Enum variant
#[derive(Debug, Clone)]
pub struct EnumVariant {
    /// Variant name
    pub name: String,
    /// Variant fields
    pub fields: StructFields,
    /// Discriminant value (if specified)
    pub discriminant: Option<i128>,
}
