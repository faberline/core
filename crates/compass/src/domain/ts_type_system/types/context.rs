use std::collections::HashMap;

use crate::domain::ts_type_system::types::{
    TsClass, TsEnum, TsInterface, TsTypeAlias, TsTypeParam,
};
use crate::type_inference::{Type, TypeVarId, Variance};

/// TypeScript type context for inference
#[derive(Debug, Clone, Default)]
pub struct TsTypeContext {
    /// Interface definitions
    pub interfaces: HashMap<String, TsInterface>,
    /// Type aliases
    pub type_aliases: HashMap<String, TsTypeAlias>,
    /// Class definitions
    pub classes: HashMap<String, TsClass>,
    /// Enum definitions
    pub enums: HashMap<String, TsEnum>,
    /// Variable types
    pub variables: HashMap<String, Type>,
    /// Type parameters in scope
    pub type_params: HashMap<String, TsTypeParam>,
}

impl TsTypeContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a type by name
    pub fn resolve_type(&self, name: &str) -> Option<Type> {
        // Check type aliases first
        if let Some(alias) = self.type_aliases.get(name) {
            return Some(alias.ty.clone());
        }

        // Check interfaces
        if let Some(iface) = self.interfaces.get(name) {
            // Convert interface to Protocol type
            let members: Vec<(String, Type)> = iface
                .properties
                .iter()
                .chain(iface.methods.iter())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            return Some(Type::Protocol {
                name: name.to_string(),
                module: None,
                members,
            });
        }

        // Check classes
        if self.classes.contains_key(name) {
            return Some(Type::ClassType {
                name: name.to_string(),
                module: None,
            });
        }

        // Check type parameters
        if let Some(param) = self.type_params.get(name) {
            let id = TypeVarId(name.len()); // Simplified ID generation
            return Some(Type::TypeVar {
                id,
                name: name.to_string(),
                bound: param.constraint.clone().map(Box::new),
                constraints: vec![],
                variance: Variance::Invariant,
            });
        }

        None
    }

    /// Register an interface
    pub fn register_interface(&mut self, iface: TsInterface) {
        self.interfaces.insert(iface.name.clone(), iface);
    }

    /// Register a class
    pub fn register_class(&mut self, class: TsClass) {
        self.classes.insert(class.name.clone(), class);
    }

    /// Register a type alias
    pub fn register_type_alias(&mut self, alias: TsTypeAlias) {
        self.type_aliases.insert(alias.name.clone(), alias);
    }
}
