use crate::domain::rust_type_system::advanced::types_match;
use crate::domain::rust_type_system::infer::RustTypeContext;
use crate::domain::rust_type_system::types::{RustType, TraitId, TraitRef};

// ============================================================================
// R2c: Associated type projections
// ============================================================================

/// Resolution context for `<T as Trait>::Assoc` projections.
pub struct ProjectionResolver<'ctx> {
    ctx: &'ctx RustTypeContext,
}

impl<'ctx> ProjectionResolver<'ctx> {
    /// Create a resolver backed by an existing `RustTypeContext`.
    pub fn new(ctx: &'ctx RustTypeContext) -> Self {
        Self { ctx }
    }

    /// Resolve an associated type projection `<concrete_type as trait_name>::assoc_name`.
    ///
    /// Looks through all impl blocks in the context for one that:
    /// 1. implements `trait_name` for `concrete_type`, and
    /// 2. provides a concrete value for `assoc_name`.
    ///
    /// Returns `None` when no matching impl is found.
    pub fn resolve_projection(
        &self,
        concrete_type: &RustType,
        trait_name: &str,
        assoc_name: &str,
    ) -> Option<RustType> {
        for imp in self.ctx.trait_impls.iter() {
            let impl_trait_name = imp
                .trait_ref
                .as_ref()
                .map(|tr| tr.name.as_str())
                .unwrap_or("");

            if impl_trait_name != trait_name {
                continue;
            }
            if !types_match(&imp.self_type, concrete_type) {
                continue;
            }

            if let Some((_, ty)) = imp.associated_types.iter().find(|(n, _)| n == assoc_name) {
                return Some(ty.clone());
            }
        }

        // Fall back: look up the associated type's default in the trait definition.
        for trait_def in self.ctx.trait_defs.values() {
            if trait_def.name != trait_name {
                continue;
            }
            if let Some(at) = trait_def
                .associated_types
                .iter()
                .find(|at| at.name == assoc_name)
            {
                return at.default.clone();
            }
        }

        None
    }

    /// Build a `RustType::Projection` node for a given type, trait, and
    /// associated type name.
    pub fn build_projection(
        &self,
        base_type: RustType,
        trait_id: TraitId,
        trait_name: impl Into<String>,
        assoc_name: impl Into<String>,
    ) -> RustType {
        let trait_ref = TraitRef {
            trait_id,
            name: trait_name.into(),
            type_args: vec![],
            lifetime_args: vec![],
        };
        RustType::Projection {
            base: Box::new(base_type),
            trait_ref: Some(trait_ref),
            name: assoc_name.into(),
        }
    }
}
