use crate::domain::rust_type_system::advanced::types_match;
use crate::domain::rust_type_system::infer::RustTypeContext;
use crate::domain::rust_type_system::types::{Lifetime, RustType, TraitBound};

// ============================================================================
// R2b: Complex trait bounds
// ============================================================================

/// An associated-type constraint within a trait bound.
///
/// Corresponds to `<Item = u32>` in `T: Iterator<Item = u32>`.
#[derive(Debug, Clone, PartialEq)]
pub struct AssocTypeConstraint {
    /// Name of the associated type (e.g. `"Item"`)
    pub name: String,
    /// Required concrete type
    pub ty: RustType,
}

/// A complex trait bound for a single type variable, combining multiple
/// `TraitBound`s and associated-type equality constraints.
///
/// Corresponds to a `where` clause predicate like:
/// `T: Iterator<Item = u32> + DoubleEndedIterator + Send + 'static`
#[derive(Debug, Clone)]
pub struct ComplexTraitBounds {
    /// The type being bounded (usually a `TypeParam`)
    pub ty: RustType,
    /// Required trait bounds
    pub trait_bounds: Vec<TraitBound>,
    /// Associated-type equality constraints
    pub assoc_constraints: Vec<AssocTypeConstraint>,
    /// Required lifetime bounds
    pub lifetime_bounds: Vec<Lifetime>,
}

/// Result of checking a type against a `ComplexTraitBounds` spec.
#[derive(Debug, Clone, PartialEq)]
pub enum BoundCheckResult {
    /// All bounds are satisfied.
    Satisfied,
    /// One or more trait bounds are not satisfied.
    MissingTraits(Vec<String>),
    /// One or more associated-type constraints are violated.
    AssocTypeMismatch {
        trait_name: String,
        assoc_name: String,
        expected: RustType,
        got: Option<RustType>,
    },
    /// Insufficient information — result is unknown.
    Unknown,
}

/// Check whether the impl blocks in `ctx` satisfy all the bounds in `spec`
/// for the concrete type `concrete_type`.
///
/// This performs a structural walk: for each required trait, it looks up
/// impl blocks in the context, verifies the impl covers the concrete type,
/// and then checks associated-type constraints.
pub fn check_complex_trait_bounds(
    ctx: &RustTypeContext,
    concrete_type: &RustType,
    spec: &ComplexTraitBounds,
) -> BoundCheckResult {
    let mut missing_traits: Vec<String> = Vec::new();

    for bound in &spec.trait_bounds {
        if bound.is_negative {
            // Negative bounds (`!Send`) are assumed satisfied unless we can
            // prove otherwise — conservative approximation.
            continue;
        }

        let trait_name = &bound.trait_ref.name;
        let impl_found = ctx.trait_impls.iter().any(|imp| {
            imp.trait_ref
                .as_ref()
                .map_or(false, |tr| &tr.name == trait_name)
                && types_match(&imp.self_type, concrete_type)
        });

        if !impl_found {
            // Check if the trait_def is a built-in auto trait (Send/Sync/Sized).
            let is_auto = ctx
                .trait_defs
                .values()
                .any(|def| def.name == *trait_name && def.is_auto);

            if !is_auto {
                missing_traits.push(trait_name.clone());
            }
        }
    }

    if !missing_traits.is_empty() {
        return BoundCheckResult::MissingTraits(missing_traits);
    }

    // Check associated-type constraints.
    for constraint in &spec.assoc_constraints {
        // Find an impl that provides this associated type.
        let assoc_ty = ctx.trait_impls.iter().find_map(|imp| {
            if types_match(&imp.self_type, concrete_type) {
                imp.associated_types
                    .iter()
                    .find(|(n, _)| n == &constraint.name)
                    .map(|(_, t)| t.clone())
            } else {
                None
            }
        });

        match assoc_ty {
            Some(ref got) if got == &constraint.ty => {}
            Some(got) => {
                // We need to find the trait name for this associated type.
                let trait_name = spec
                    .trait_bounds
                    .first()
                    .map(|b| b.trait_ref.name.clone())
                    .unwrap_or_else(|| "unknown".to_string());

                return BoundCheckResult::AssocTypeMismatch {
                    trait_name,
                    assoc_name: constraint.name.clone(),
                    expected: constraint.ty.clone(),
                    got: Some(got),
                };
            }
            None => {
                // Associated type not found in any impl; if the trait provides
                // a default we consider it satisfied.
            }
        }
    }

    BoundCheckResult::Satisfied
}
