use crate::domain::type_system::ty::{Type, Variance};

/// Information extracted from a TypeVar(...) call
#[derive(Debug, Clone)]
pub struct TypeVarInfo {
    /// The name of the TypeVar (e.g., "T" from TypeVar("T", ...))
    pub name: String,
    /// Variance: covariant, contravariant, or invariant
    pub variance: Variance,
    /// Optional upper bound
    pub bound: Option<Type>,
    /// Type constraints (TypeVar can only be one of these types)
    pub constraints: Vec<Type>,
}
