//! TypeScript-specific type definitions
//!
//! Extends the core type system with TypeScript-specific constructs.

use std::collections::HashMap;

use crate::type_inference::Type;

mod assignability;
mod context;
mod ts_enum;
mod type_operators;

pub use assignability::is_assignable_to;
pub use context::TsTypeContext;
pub use ts_enum::{TsEnum, TsEnumValue};
pub use type_operators::{
    MappedTypeModifier, TemplatePart, TsConditionalType, TsMappedType, TsTemplateLiteralType,
};

/// TypeScript interface definition
#[derive(Debug, Clone, PartialEq)]
pub struct TsInterface {
    /// Interface name
    pub name: String,
    /// Generic type parameters
    pub type_params: Vec<TsTypeParam>,
    /// Required properties (name -> type)
    pub properties: HashMap<String, Type>,
    /// Optional properties (name -> type)
    pub optional_properties: HashMap<String, Type>,
    /// Methods (name -> signature)
    pub methods: HashMap<String, Type>,
    /// Index signature: [key: T]: U
    pub index_signature: Option<(Type, Type)>,
    /// Extended interfaces
    pub extends: Vec<String>,
}

impl TsInterface {
    pub fn new(name: String) -> Self {
        Self {
            name,
            type_params: Vec::new(),
            properties: HashMap::new(),
            optional_properties: HashMap::new(),
            methods: HashMap::new(),
            index_signature: None,
            extends: Vec::new(),
        }
    }

    /// Get all members (including from extended interfaces)
    pub fn all_properties(&self) -> Vec<(String, Type, bool)> {
        let mut props: Vec<(String, Type, bool)> = self
            .properties
            .iter()
            .map(|(k, v)| (k.clone(), v.clone(), true))
            .collect();
        props.extend(
            self.optional_properties
                .iter()
                .map(|(k, v)| (k.clone(), v.clone(), false)),
        );
        props
    }
}

/// TypeScript type parameter
#[derive(Debug, Clone, PartialEq)]
pub struct TsTypeParam {
    /// Parameter name (e.g., "T", "K")
    pub name: String,
    /// Constraint: T extends SomeType
    pub constraint: Option<Type>,
    /// Default type: T = DefaultType
    pub default: Option<Type>,
}

impl TsTypeParam {
    pub fn new(name: String) -> Self {
        Self {
            name,
            constraint: None,
            default: None,
        }
    }

    pub fn with_constraint(mut self, constraint: Type) -> Self {
        self.constraint = Some(constraint);
        self
    }

    pub fn with_default(mut self, default: Type) -> Self {
        self.default = Some(default);
        self
    }
}

/// TypeScript type alias
#[derive(Debug, Clone, PartialEq)]
pub struct TsTypeAlias {
    /// Alias name
    pub name: String,
    /// Type parameters
    pub type_params: Vec<TsTypeParam>,
    /// The aliased type
    pub ty: Type,
}

/// TypeScript class definition
#[derive(Debug, Clone, PartialEq)]
pub struct TsClass {
    /// Class name
    pub name: String,
    /// Generic type parameters
    pub type_params: Vec<TsTypeParam>,
    /// Instance properties
    pub properties: HashMap<String, TsProperty>,
    /// Instance methods
    pub methods: HashMap<String, Type>,
    /// Static properties
    pub static_properties: HashMap<String, Type>,
    /// Static methods
    pub static_methods: HashMap<String, Type>,
    /// Constructor signature
    pub constructor: Option<Type>,
    /// Base class
    pub extends: Option<String>,
    /// Implemented interfaces
    pub implements: Vec<String>,
}

impl TsClass {
    pub fn new(name: String) -> Self {
        Self {
            name,
            type_params: Vec::new(),
            properties: HashMap::new(),
            methods: HashMap::new(),
            static_properties: HashMap::new(),
            static_methods: HashMap::new(),
            constructor: None,
            extends: None,
            implements: Vec::new(),
        }
    }
}

/// TypeScript property with modifiers
#[derive(Debug, Clone, PartialEq)]
pub struct TsProperty {
    /// Property type
    pub ty: Type,
    /// Is optional?
    pub optional: bool,
    /// Is readonly?
    pub readonly: bool,
    /// Visibility (public, private, protected)
    pub visibility: Visibility,
}

impl TsProperty {
    pub fn new(ty: Type) -> Self {
        Self {
            ty,
            optional: false,
            readonly: false,
            visibility: Visibility::Public,
        }
    }
}

/// TypeScript visibility modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Public,
    Private,
    Protected,
}

#[cfg(test)]
mod tests;
