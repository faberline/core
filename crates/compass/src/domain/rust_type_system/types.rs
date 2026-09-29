//! Rust-specific type system extensions
//!
//! This module provides Rust-specific type constructs for the Lens type system,
//! including traits, lifetimes, and Rust-specific type inference.

use crate::type_inference::{Type, TypeVarId};

mod adt_def;
mod impl_block;
mod lifetime;
mod trait_def;

pub use adt_def::{EnumDef, EnumVariant, StructDef, StructField, StructFields};
pub use impl_block::{ImplBlock, ImplMethod, Visibility};
pub use lifetime::{Lifetime, LifetimeId};
pub use trait_def::{
    AssociatedType, SelfParam, TraitBound, TraitDef, TraitId, TraitMethod, TraitRef,
};

// ============================================================================
// Rust Type Extensions
// ============================================================================

/// Rust-specific type representation
#[derive(Debug, Clone, PartialEq)]
pub enum RustType {
    /// Primitive types
    Unit,
    Bool,
    Char,
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    F32,
    F64,
    Str,

    /// Reference type (&T or &mut T)
    Reference {
        lifetime: Option<Lifetime>,
        mutable: bool,
        inner: Box<RustType>,
    },

    /// Raw pointer (*const T or *mut T)
    RawPointer {
        mutable: bool,
        inner: Box<RustType>,
    },

    /// Slice type ([T])
    Slice(Box<RustType>),

    /// Array type ([T; N])
    Array {
        element: Box<RustType>,
        size: usize,
    },

    /// Tuple type ((A, B, C))
    Tuple(Vec<RustType>),

    /// Function pointer (fn(A, B) -> C)
    FnPointer {
        params: Vec<RustType>,
        return_type: Box<RustType>,
        is_unsafe: bool,
        abi: Option<String>,
    },

    /// Closure type (impl Fn(A) -> B)
    Closure {
        kind: ClosureKind,
        params: Vec<RustType>,
        return_type: Box<RustType>,
    },

    /// Named type (struct, enum, type alias)
    Named {
        name: String,
        module: Option<String>,
        type_args: Vec<RustType>,
        lifetime_args: Vec<Lifetime>,
    },

    /// Trait object (dyn Trait + 'a)
    TraitObject {
        bounds: Vec<TraitBound>,
        lifetime: Option<Lifetime>,
    },

    /// Impl trait (impl Trait)
    ImplTrait {
        bounds: Vec<TraitBound>,
    },

    /// Type parameter (generic T)
    TypeParam {
        id: TypeVarId,
        name: String,
        bounds: Vec<TraitBound>,
    },

    /// Associated type projection (T::Item)
    Projection {
        base: Box<RustType>,
        trait_ref: Option<TraitRef>,
        name: String,
    },

    /// Never type (!)
    Never,

    /// Inferred type (_)
    Infer,

    /// Error type (for error recovery)
    Error,
}

/// Closure kind (Fn, FnMut, FnOnce)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosureKind {
    Fn,
    FnMut,
    FnOnce,
}

/// Rust type parameter with bounds
#[derive(Debug, Clone)]
pub struct RustTypeParam {
    /// Parameter name
    pub name: String,
    /// Type variable ID
    pub id: TypeVarId,
    /// Trait bounds
    pub bounds: Vec<TraitBound>,
    /// Default type
    pub default: Option<RustType>,
}

/// Rust function/method parameter
#[derive(Debug, Clone)]
pub struct RustParam {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub ty: RustType,
    /// Whether this is a pattern parameter
    pub is_pattern: bool,
}

/// Where clause predicate
#[derive(Debug, Clone)]
pub enum WherePredicate {
    /// Type bound (T: Trait)
    TypeBound {
        ty: RustType,
        bounds: Vec<TraitBound>,
    },
    /// Lifetime bound ('a: 'b)
    LifetimeBound {
        lifetime: Lifetime,
        bounds: Vec<Lifetime>,
    },
    /// Type equality (T::Item = U)
    TypeEquality { projection: RustType, ty: RustType },
}

// ============================================================================
// Conversion Utilities
// ============================================================================

impl RustType {
    /// Convert to the generic Type enum (for cross-language operations)
    pub fn to_generic_type(&self) -> Type {
        match self {
            RustType::Unit => Type::None,
            RustType::Bool => Type::Bool,
            RustType::I8
            | RustType::I16
            | RustType::I32
            | RustType::I64
            | RustType::I128
            | RustType::Isize
            | RustType::U8
            | RustType::U16
            | RustType::U32
            | RustType::U64
            | RustType::U128
            | RustType::Usize => Type::Int,
            RustType::F32 | RustType::F64 => Type::Float,
            RustType::Str | RustType::Char => Type::Str,
            RustType::Reference { inner, .. } => inner.to_generic_type(),
            RustType::Slice(inner) => Type::list(inner.to_generic_type()),
            RustType::Array { element, .. } => Type::list(element.to_generic_type()),
            RustType::Tuple(elements) => {
                Type::Tuple(elements.iter().map(|t| t.to_generic_type()).collect())
            }
            RustType::Named {
                name,
                module,
                type_args,
                ..
            } => Type::Instance {
                name: name.clone(),
                module: module.clone(),
                type_args: type_args.iter().map(|t| t.to_generic_type()).collect(),
            },
            RustType::Never => Type::Never,
            RustType::Infer => Type::Unknown,
            RustType::Error => Type::Error,
            _ => Type::Any, // Complex Rust types map to Any for now
        }
    }

    /// Check if this type implements a trait
    pub fn implements_trait(&self, trait_ref: &TraitRef, impls: &[ImplBlock]) -> bool {
        impls.iter().any(|impl_block| {
            // Check if this impl is for the requested trait
            if let Some(ref impl_trait) = impl_block.trait_ref {
                if impl_trait.trait_id == trait_ref.trait_id {
                    // Check if the impl's self type matches this type
                    return self.matches_impl_type(&impl_block.self_type);
                }
            }
            false
        })
    }

    /// Check if this type matches an impl's self type (for trait resolution)
    fn matches_impl_type(&self, pattern: &RustType) -> bool {
        match (pattern, self) {
            // Exact match
            (a, b) if a == b => true,

            // Type parameter in impl matches any concrete type
            (RustType::TypeParam { .. }, _) => true,

            // Named types must match name and recursively match type args
            (
                RustType::Named {
                    name: n1,
                    type_args: a1,
                    ..
                },
                RustType::Named {
                    name: n2,
                    type_args: a2,
                    ..
                },
            ) => {
                n1 == n2
                    && a1.len() == a2.len()
                    && a1
                        .iter()
                        .zip(a2.iter())
                        .all(|(p, c)| c.matches_impl_type(p))
            }

            // Reference types must match mutability and inner type
            (
                RustType::Reference {
                    mutable: m1,
                    inner: i1,
                    ..
                },
                RustType::Reference {
                    mutable: m2,
                    inner: i2,
                    ..
                },
            ) => m1 == m2 && i2.matches_impl_type(i1),

            // Slices
            (RustType::Slice(e1), RustType::Slice(e2)) => e2.matches_impl_type(e1),

            // Arrays must match element type and size
            (
                RustType::Array {
                    element: e1,
                    size: s1,
                },
                RustType::Array {
                    element: e2,
                    size: s2,
                },
            ) => s1 == s2 && e2.matches_impl_type(e1),

            // Tuples must match length and element types
            (RustType::Tuple(t1), RustType::Tuple(t2)) => {
                t1.len() == t2.len()
                    && t1
                        .iter()
                        .zip(t2.iter())
                        .all(|(p, c)| c.matches_impl_type(p))
            }

            // Primitives match exactly (handled by first arm)
            _ => false,
        }
    }

    /// Get the size of this type (for arrays)
    pub fn array_size(&self) -> Option<usize> {
        match self {
            RustType::Array { size, .. } => Some(*size),
            _ => None,
        }
    }
}

impl Default for RustType {
    fn default() -> Self {
        RustType::Infer
    }
}

#[cfg(test)]
mod tests;
