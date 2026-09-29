use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::domain::cross_file::binding::{
    GenericKey, ProtocolDef, TypeBinding, TypeId, TypeVarInfo,
};
use crate::type_inference::{ClassInfo, Type};

// ============================================================================
// Type Context for Cross-File Tracking
// ============================================================================

/// Cross-file type context for tracking type information across modules.
#[derive(Debug, Clone)]
pub struct TypeContext {
    /// Type bindings by file and symbol
    pub(crate) bindings: HashMap<PathBuf, HashMap<String, TypeBinding>>,
    /// Type variables in scope
    type_vars: HashMap<String, TypeVarInfo>,
    /// Protocol definitions
    protocols: HashMap<String, ProtocolDef>,
    /// Generic instantiations cache
    generic_cache: HashMap<GenericKey, Type>,
    /// Recursive type detection
    recursive_guard: HashSet<TypeId>,
    /// Class information (for protocol conformance checking)
    class_info: HashMap<String, ClassInfo>,
}

impl TypeContext {
    /// Create a new type context.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            type_vars: HashMap::new(),
            protocols: HashMap::new(),
            generic_cache: HashMap::new(),
            recursive_guard: HashSet::new(),
            class_info: HashMap::new(),
        }
    }

    /// Add a type binding.
    pub fn add_binding(&mut self, file: PathBuf, binding: TypeBinding) {
        self.bindings
            .entry(file)
            .or_default()
            .insert(binding.symbol.clone(), binding);
    }

    /// Get a type binding.
    pub fn get_binding(&self, file: &PathBuf, symbol: &str) -> Option<&TypeBinding> {
        self.bindings.get(file)?.get(symbol)
    }

    /// Resolve a type across files.
    pub fn resolve_type(&self, symbol: &str, from_file: &PathBuf) -> Option<&Type> {
        // First check current file
        if let Some(binding) = self.get_binding(from_file, symbol) {
            return Some(&binding.ty);
        }

        // Then check all files for exported symbols
        for (_, bindings) in &self.bindings {
            if let Some(binding) = bindings.get(symbol) {
                if binding.is_exported {
                    return Some(&binding.ty);
                }
            }
        }

        None
    }

    /// Register a TypeVar.
    pub fn register_type_var(&mut self, info: TypeVarInfo) {
        self.type_vars.insert(info.name.clone(), info);
    }

    /// Get TypeVar info.
    pub fn get_type_var(&self, name: &str) -> Option<&TypeVarInfo> {
        self.type_vars.get(name)
    }

    /// Register a protocol.
    pub fn register_protocol(&mut self, protocol: ProtocolDef) {
        self.protocols.insert(protocol.name.clone(), protocol);
    }

    /// Check if a type satisfies a protocol (structural typing).
    pub fn satisfies_protocol(&self, ty: &Type, protocol_name: &str) -> bool {
        let protocol = match self.protocols.get(protocol_name) {
            Some(p) => p,
            None => return false,
        };

        // Check all required methods and attributes
        // This is a placeholder - full implementation would inspect the type
        self.check_protocol_conformance(ty, protocol)
    }

    fn check_protocol_conformance(&self, ty: &Type, protocol: &ProtocolDef) -> bool {
        // Extract class name from Type
        let class_name = match ty {
            Type::Instance { name, .. } => name,
            Type::ClassType { name, .. } => name,
            _ => return false, // Non-class types can't implement protocols
        };

        // Get class information
        let class_info = match self.class_info.get(class_name) {
            Some(info) => info,
            None => return false, // Unknown class, can't check
        };

        // Check all required methods in the protocol
        for (method_name, required_sig) in &protocol.methods {
            match class_info.methods.get(method_name) {
                Some(class_method_ty) => {
                    // Check if method signature is compatible
                    if !self.is_signature_compatible(
                        class_method_ty,
                        &required_sig.return_type,
                        &required_sig.params,
                    ) {
                        return false;
                    }
                }
                None => return false, // Required method not found
            }
        }

        // Check all required attributes in the protocol
        for (attr_name, required_ty) in &protocol.attributes {
            match class_info.attributes.get(attr_name) {
                Some(class_attr_ty) => {
                    // Check if attribute type is compatible
                    if !self.is_type_compatible(class_attr_ty, required_ty) {
                        return false;
                    }
                }
                None => return false, // Required attribute not found
            }
        }

        // Check parent protocols recursively
        for parent_name in &protocol.parents {
            if let Some(parent_protocol) = self.protocols.get(parent_name) {
                if !self.check_protocol_conformance(ty, parent_protocol) {
                    return false;
                }
            }
        }

        true
    }

    /// Check if a method signature is compatible with requirements.
    fn is_signature_compatible(
        &self,
        method_ty: &Type,
        required_ret: &Type,
        required_params: &[(String, Type)],
    ) -> bool {
        // Extract callable signature from method type
        let (actual_params, actual_ret) = match method_ty {
            Type::Callable { params, ret } => (params, ret.as_ref()),
            _ => return false,
        };

        // Check return type compatibility (covariant)
        if !self.is_type_compatible(actual_ret, required_ret) {
            return false;
        }

        // Check parameter count
        if actual_params.len() < required_params.len() {
            return false;
        }

        // Check parameter types (contravariant)
        for (i, (_, required_param_ty)) in required_params.iter().enumerate() {
            if let Some(actual_param) = actual_params.get(i) {
                // Parameters are contravariant: required type must be subtype of actual
                if !self.is_type_compatible(required_param_ty, &actual_param.ty) {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    /// Check if two types are compatible (basic structural equality).
    fn is_type_compatible(&self, actual: &Type, required: &Type) -> bool {
        use Type::*;

        match (actual, required) {
            // Exact matches
            (Never, Never)
            | (None, None)
            | (Bool, Bool)
            | (Int, Int)
            | (Float, Float)
            | (Str, Str)
            | (Bytes, Bytes) => true,

            // Any accepts everything
            (_, Any) | (Any, _) => true,

            // Unknown can match anything (inference incomplete)
            (Unknown, _) | (_, Unknown) => true,

            // Lists - check element type
            (List(a), List(b)) => self.is_type_compatible(a, b),

            // Dicts - check key and value types
            (Dict(k1, v1), Dict(k2, v2)) => {
                self.is_type_compatible(k1, k2) && self.is_type_compatible(v1, v2)
            }

            // Sets - check element type
            (Set(a), Set(b)) => self.is_type_compatible(a, b),

            // Tuples - check all element types
            (Tuple(a), Tuple(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|(x, y)| self.is_type_compatible(x, y))
            }

            // Optional types
            (Optional(a), Optional(b)) => self.is_type_compatible(a, b),
            (Optional(a), b) => self.is_type_compatible(a, b),
            (a, Optional(b)) => self.is_type_compatible(a, b),

            // Unions - actual must be subset of required
            (Union(actuals), Union(requireds)) => actuals
                .iter()
                .all(|a| requireds.iter().any(|r| self.is_type_compatible(a, r))),
            (actual, Union(requireds)) => {
                requireds.iter().any(|r| self.is_type_compatible(actual, r))
            }

            // Instances - check name compatibility
            (Instance { name: n1, .. }, Instance { name: n2, .. }) => n1 == n2,

            // Class types
            (ClassType { name: n1, .. }, ClassType { name: n2, .. }) => n1 == n2,

            // Callables - check signature compatibility
            (
                Callable {
                    params: p1,
                    ret: r1,
                },
                Callable {
                    params: p2,
                    ret: r2,
                },
            ) => {
                // Return types are covariant
                self.is_type_compatible(r1, r2) &&
                // Parameters are contravariant (and must match count)
                p1.len() == p2.len() &&
                p1.iter().zip(p2.iter()).all(|(a, b)| self.is_type_compatible(&b.ty, &a.ty))
            }

            // Default: not compatible
            _ => false,
        }
    }

    /// Cache a generic instantiation.
    pub fn cache_generic(&mut self, key: GenericKey, ty: Type) {
        self.generic_cache.insert(key, ty);
    }

    /// Get cached generic instantiation.
    pub fn get_cached_generic(&self, key: &GenericKey) -> Option<&Type> {
        self.generic_cache.get(key)
    }

    /// Enter recursive type checking (returns false if already checking this type).
    pub fn enter_recursive(&mut self, type_id: TypeId) -> bool {
        self.recursive_guard.insert(type_id)
    }

    /// Exit recursive type checking.
    pub fn exit_recursive(&mut self, type_id: TypeId) {
        self.recursive_guard.remove(&type_id);
    }

    /// Check if currently checking a recursive type.
    pub fn is_recursive(&self, type_id: TypeId) -> bool {
        self.recursive_guard.contains(&type_id)
    }

    /// Add class information.
    pub fn add_class_info(&mut self, name: String, info: ClassInfo) {
        self.class_info.insert(name, info);
    }

    /// Get class information.
    pub fn get_class_info(&self, name: &str) -> Option<&ClassInfo> {
        self.class_info.get(name)
    }

    /// Get mutable class information.
    pub fn get_class_info_mut(&mut self, name: &str) -> Option<&mut ClassInfo> {
        self.class_info.get_mut(name)
    }

    /// Add a protocol definition.
    pub fn add_protocol(&mut self, name: String, protocol: ProtocolDef) {
        self.protocols.insert(name, protocol);
    }

    /// Get a protocol definition.
    pub fn get_protocol(&self, name: &str) -> Option<&ProtocolDef> {
        self.protocols.get(name)
    }
}

impl Default for TypeContext {
    fn default() -> Self {
        Self::new()
    }
}
