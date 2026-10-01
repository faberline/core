use crate::domain::rust_type_system::types::{
    Lifetime, RustParam, RustType, RustTypeParam, WherePredicate,
};

// ============================================================================
// Trait Types
// ============================================================================

/// Unique identifier for traits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraitId(pub usize);

/// Rust trait definition
#[derive(Debug, Clone)]
pub struct TraitDef {
    /// Trait identifier
    pub id: TraitId,
    /// Trait name
    pub name: String,
    /// Module path
    pub module: Option<String>,
    /// Generic type parameters
    pub type_params: Vec<RustTypeParam>,
    /// Supertraits (trait bounds)
    pub supertraits: Vec<TraitBound>,
    /// Associated types
    pub associated_types: Vec<AssociatedType>,
    /// Required methods
    pub required_methods: Vec<TraitMethod>,
    /// Provided methods (with default impl)
    pub provided_methods: Vec<TraitMethod>,
    /// Whether this is an auto trait (Send, Sync, etc.)
    pub is_auto: bool,
    /// Whether this is a marker trait (no methods)
    pub is_marker: bool,
}

/// Trait bound (e.g., T: Clone + Send)
#[derive(Debug, Clone, PartialEq)]
pub struct TraitBound {
    /// The trait being bounded
    pub trait_ref: TraitRef,
    /// Whether this is a negative bound (T: !Sync)
    pub is_negative: bool,
    /// Higher-ranked lifetime bounds (for<'a> T: Trait<'a>)
    pub higher_ranked_lifetimes: Vec<Lifetime>,
}

/// Reference to a trait with type arguments
#[derive(Debug, Clone, PartialEq)]
pub struct TraitRef {
    /// Trait identifier
    pub trait_id: TraitId,
    /// Trait name (for display)
    pub name: String,
    /// Type arguments
    pub type_args: Vec<RustType>,
    /// Lifetime arguments
    pub lifetime_args: Vec<Lifetime>,
}

/// Associated type in a trait
#[derive(Debug, Clone)]
pub struct AssociatedType {
    /// Name of the associated type
    pub name: String,
    /// Bounds on the associated type
    pub bounds: Vec<TraitBound>,
    /// Default type (if provided)
    pub default: Option<RustType>,
}

/// Trait method signature
#[derive(Debug, Clone)]
pub struct TraitMethod {
    /// Method name
    pub name: String,
    /// Generic parameters
    pub type_params: Vec<RustTypeParam>,
    /// Self parameter type
    pub self_param: Option<SelfParam>,
    /// Other parameters
    pub params: Vec<RustParam>,
    /// Return type
    pub return_type: RustType,
    /// Where clause bounds
    pub where_bounds: Vec<WherePredicate>,
    /// Whether this is unsafe
    pub is_unsafe: bool,
    /// Whether this is async
    pub is_async: bool,
}

/// Self parameter in a method
#[derive(Debug, Clone, PartialEq)]
pub enum SelfParam {
    /// self
    Value,
    /// &self
    Ref(Option<Lifetime>),
    /// &mut self
    RefMut(Option<Lifetime>),
    /// self: Box<Self>
    Explicit(Box<RustType>),
}
