use crate::domain::rust_type_system::types::{
    RustType, RustTypeParam, TraitMethod, TraitRef, WherePredicate,
};

// ============================================================================
// Impl Block Types
// ============================================================================

/// Impl block representation
#[derive(Debug, Clone)]
pub struct ImplBlock {
    /// Type parameters
    pub type_params: Vec<RustTypeParam>,
    /// Trait being implemented (if trait impl)
    pub trait_ref: Option<TraitRef>,
    /// Type being implemented for
    pub self_type: RustType,
    /// Where clause
    pub where_bounds: Vec<WherePredicate>,
    /// Methods in this impl
    pub methods: Vec<ImplMethod>,
    /// Associated types
    pub associated_types: Vec<(String, RustType)>,
    /// Associated constants
    pub associated_consts: Vec<(String, RustType)>,
    /// Whether this is a negative impl
    pub is_negative: bool,
    /// Whether this is unsafe
    pub is_unsafe: bool,
}

/// Method in an impl block
#[derive(Debug, Clone)]
pub struct ImplMethod {
    /// Method name
    pub name: String,
    /// Visibility
    pub visibility: Visibility,
    /// Method signature
    pub signature: TraitMethod,
    /// Whether this is a default impl
    pub is_default: bool,
}

/// Visibility modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Crate,
    Super,
    Private,
    Restricted, // pub(in path)
}
