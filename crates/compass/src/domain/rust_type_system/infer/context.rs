use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::rust_type_system::types::{
    EnumDef, ImplBlock, Lifetime, LifetimeId, RustType, StructDef, TraitDef, TraitId,
};
use crate::type_inference::TypeVarId;

// ============================================================================
// Type Inference Context
// ============================================================================

/// Context for Rust type inference
#[derive(Debug, Clone)]
pub struct RustTypeContext {
    /// Current scope's type bindings
    pub type_bindings: HashMap<String, RustType>,
    /// Current scope's lifetime bindings
    pub lifetime_bindings: HashMap<String, Lifetime>,
    /// Available trait impls
    pub trait_impls: Vec<Arc<ImplBlock>>,
    /// Known trait definitions
    pub trait_defs: HashMap<TraitId, Arc<TraitDef>>,
    /// Known struct definitions
    pub struct_defs: HashMap<String, Arc<StructDef>>,
    /// Known enum definitions
    pub enum_defs: HashMap<String, Arc<EnumDef>>,
    /// Type variable counter
    type_var_counter: usize,
    /// Lifetime counter
    lifetime_counter: usize,
}

impl RustTypeContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self {
            type_bindings: HashMap::new(),
            lifetime_bindings: HashMap::new(),
            trait_impls: Vec::new(),
            trait_defs: HashMap::new(),
            struct_defs: HashMap::new(),
            enum_defs: HashMap::new(),
            type_var_counter: 0,
            lifetime_counter: 0,
        }
    }

    /// Create a new type variable
    pub fn fresh_type_var(&mut self, name: impl Into<String>) -> RustType {
        let id = TypeVarId(self.type_var_counter);
        self.type_var_counter += 1;
        RustType::TypeParam {
            id,
            name: name.into(),
            bounds: vec![],
        }
    }

    /// Create a new lifetime
    pub fn fresh_lifetime(&mut self) -> Lifetime {
        let id = LifetimeId(self.lifetime_counter);
        self.lifetime_counter += 1;
        Lifetime::Inferred(id)
    }

    /// Look up a type binding
    pub fn lookup_type(&self, name: &str) -> Option<RustType> {
        self.type_bindings.get(name).cloned()
    }

    /// Add a type binding
    pub fn bind_type(&mut self, name: String, ty: RustType) {
        self.type_bindings.insert(name, ty);
    }

    /// Look up a lifetime binding
    pub fn lookup_lifetime(&self, name: &str) -> Option<Lifetime> {
        self.lifetime_bindings.get(name).cloned()
    }

    /// Add a lifetime binding
    pub fn bind_lifetime(&mut self, name: String, lifetime: Lifetime) {
        self.lifetime_bindings.insert(name, lifetime);
    }

    /// Create a child context (for nested scopes)
    pub fn child(&self) -> Self {
        Self {
            type_bindings: self.type_bindings.clone(),
            lifetime_bindings: self.lifetime_bindings.clone(),
            trait_impls: self.trait_impls.clone(),
            trait_defs: self.trait_defs.clone(),
            struct_defs: self.struct_defs.clone(),
            enum_defs: self.enum_defs.clone(),
            type_var_counter: self.type_var_counter,
            lifetime_counter: self.lifetime_counter,
        }
    }
}

impl Default for RustTypeContext {
    fn default() -> Self {
        Self::new()
    }
}
