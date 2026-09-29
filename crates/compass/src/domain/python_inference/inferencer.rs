//! Type inference engine for Python

mod classes;
mod compound;
mod expressions;
mod functions;
mod special_classes;
mod type_vars;

use std::collections::HashMap;

use tree_sitter::Node;

use crate::domain::type_system::builtins::add_builtins;
use crate::domain::type_system::class_info::ClassInfo;
use crate::domain::type_system::ty::{Type, TypeVarId, Variance};
use crate::domain::type_system::type_env::TypeEnv;
use crate::type_inference::imports::parse_import;
use crate::type_inference::{FrameworkRegistry, ImportResolver, StubLoader};

/// Type inferencer for Python code
pub struct TypeInferencer<'a> {
    /// Source code
    source: &'a str,
    /// Type environment
    env: TypeEnv,
    /// Class registry (class name -> class info)
    classes: HashMap<String, ClassInfo>,
    /// TypeVar registry (name -> type)
    type_vars: HashMap<String, Type>,
    /// Counter for generating fresh type variables
    next_type_var: usize,
    /// Type overrides from narrowing (checked before env)
    type_overrides: Option<HashMap<String, Type>>,
    /// Stub loader for builtin/typing/collections stubs
    #[allow(dead_code)]
    stubs: StubLoader,
    /// Import resolver for module resolution
    resolver: ImportResolver,
    /// Overloaded function signatures (name -> list of Callable signatures)
    overload_signatures: HashMap<String, Vec<Type>>,
    /// Current class name (for Self type resolution)
    current_class: Option<String>,
    /// Framework type providers
    framework_registry: FrameworkRegistry,
}

impl<'a> TypeInferencer<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut env = TypeEnv::new();
        // Add builtins
        add_builtins(&mut env);

        // Initialize stubs and resolver
        let mut stubs = StubLoader::new();
        stubs.load_builtins();

        let mut resolver = ImportResolver::new();
        for (path, info) in stubs.modules() {
            resolver.register_module(path, info.clone());
        }

        Self {
            source,
            env,
            classes: HashMap::new(),
            type_vars: HashMap::new(),
            next_type_var: 0,
            type_overrides: None,
            stubs,
            resolver,
            overload_signatures: HashMap::new(),
            current_class: None,
            framework_registry: FrameworkRegistry::new(),
        }
    }

    /// Get a mutable reference to the framework registry for configuration.
    pub fn framework_registry_mut(&mut self) -> &mut FrameworkRegistry {
        &mut self.framework_registry
    }

    /// Add a type binding to the environment (useful for testing).
    pub fn bind_type(&mut self, name: String, ty: Type) {
        self.env.bind(name, ty);
    }

    /// Set type overrides from narrowing (used for control flow analysis)
    pub fn set_type_overrides(&mut self, overrides: HashMap<String, Type>) {
        self.type_overrides = Some(overrides);
    }

    /// Clear type overrides
    pub fn clear_type_overrides(&mut self) {
        self.type_overrides = None;
    }

    /// Register a TypeVar definition
    pub fn register_type_var(&mut self, name: &str, bound: Option<Type>, constraints: Vec<Type>) {
        self.register_type_var_with_variance(name, bound, constraints, Variance::Invariant);
    }

    /// Register a TypeVar definition with specific variance
    pub fn register_type_var_with_variance(
        &mut self,
        name: &str,
        bound: Option<Type>,
        constraints: Vec<Type>,
        variance: Variance,
    ) {
        let id = TypeVarId(self.next_type_var);
        self.next_type_var += 1;
        let tv = Type::TypeVar {
            id,
            name: name.to_string(),
            bound: bound.map(Box::new),
            constraints,
            variance,
        };
        self.type_vars.insert(name.to_string(), tv.clone());
        self.env.bind(name.to_string(), tv);
    }

    /// Look up a TypeVar by name
    pub fn get_type_var(&self, name: &str) -> Option<&Type> {
        self.type_vars.get(name)
    }

    /// Instantiate a generic type with concrete type arguments
    pub fn instantiate_generic(&self, generic_type: &Type, type_args: &[Type]) -> Type {
        let type_var_ids = generic_type.type_vars();
        if type_var_ids.len() != type_args.len() {
            return Type::Error; // Wrong number of type args
        }

        let substitutions: HashMap<TypeVarId, Type> = type_var_ids
            .into_iter()
            .zip(type_args.iter().cloned())
            .collect();

        generic_type.substitute(&substitutions)
    }

    /// Get class info by name
    pub fn get_class(&self, name: &str) -> Option<&ClassInfo> {
        self.classes.get(name)
    }

    /// Get attribute type with inheritance support
    /// Walks up the inheritance chain to find the attribute
    pub fn get_attribute_recursive(&self, class_name: &str, attr_name: &str) -> Option<Type> {
        if let Some(class_info) = self.classes.get(class_name) {
            // First check the current class
            if let Some(ty) = class_info.get_attribute(attr_name) {
                return Some(ty.clone());
            }
            // Then check base classes (in order)
            for base_name in &class_info.bases {
                if let Some(ty) = self.get_attribute_recursive(base_name, attr_name) {
                    return Some(ty);
                }
            }
        }
        None
    }

    /// Get class-level attribute (class vars, methods) with inheritance support
    fn get_class_attribute_recursive(&self, class_name: &str, attr_name: &str) -> Option<Type> {
        if let Some(class_info) = self.classes.get(class_name) {
            // First check class variables
            if let Some(ty) = class_info.class_vars.get(attr_name) {
                return Some(ty.clone());
            }
            // Then check methods
            if let Some(ty) = class_info.methods.get(attr_name) {
                return Some(ty.clone());
            }
            // Then check base classes
            for base_name in &class_info.bases {
                if let Some(ty) = self.get_class_attribute_recursive(base_name, attr_name) {
                    return Some(ty);
                }
            }
        }
        None
    }

    /// Check if a class is a subclass of another (including self)
    pub fn is_subclass(&self, child: &str, parent: &str) -> bool {
        if child == parent {
            return true;
        }
        if let Some(class_info) = self.classes.get(child) {
            for base_name in &class_info.bases {
                if self.is_subclass(base_name, parent) {
                    return true;
                }
            }
        }
        false
    }

    /// Generate a fresh type variable (invariant by default)
    #[allow(dead_code)]
    fn fresh_type_var(&mut self, name: &str) -> Type {
        let id = TypeVarId(self.next_type_var);
        self.next_type_var += 1;
        Type::TypeVar {
            id,
            name: name.to_string(),
            bound: None,
            constraints: vec![],
            variance: Variance::Invariant,
        }
    }

    /// Get the text of a node
    fn node_text(&self, node: &Node) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    /// Bind a variable to a type based on assignment
    pub fn bind_assignment(&mut self, target: &Node, value_type: Type) {
        if target.kind() == "identifier" {
            let name = self.node_text(target).to_string();
            self.env.bind(name, value_type);
        }
    }

    /// Get the current environment (for testing)
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// Get all variable types from the current environment
    pub fn get_env_types(&self) -> HashMap<String, Type> {
        self.env.get_all_types()
    }

    /// Analyze an import statement and add imported names to the environment
    pub fn analyze_import(&mut self, node: &Node) {
        if let Some(import) = parse_import(self.source, node) {
            let resolved = self.resolver.resolve_import(&import);
            for (name, ty) in resolved {
                self.env.bind(name, ty);
            }
        }
    }
}

#[cfg(test)]
mod tests;
