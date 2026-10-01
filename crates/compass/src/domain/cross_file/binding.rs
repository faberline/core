use std::collections::HashMap;
use std::path::PathBuf;

use crate::type_inference::Type;

/// Unique identifier for types in cross-file tracking.
pub type TypeId = usize;

/// A type binding with source information.
#[derive(Debug, Clone)]
pub struct TypeBinding {
    /// The inferred type
    pub ty: Type,
    /// Source file
    pub source_file: PathBuf,
    /// Symbol name
    pub symbol: String,
    /// Line number
    pub line: u32,
    /// Whether this is exported
    pub is_exported: bool,
    /// Dependencies (other symbols this type depends on)
    pub dependencies: Vec<String>,
    /// Whether this binding was propagated from another file (R3).
    pub is_propagated: bool,
}

/// TypeVar information for generics.
#[derive(Debug, Clone)]
pub struct TypeVarInfo {
    /// TypeVar name
    pub name: String,
    /// Bound type (if any)
    pub bound: Option<Type>,
    /// Constraints
    pub constraints: Vec<Type>,
    /// Covariant
    pub covariant: bool,
    /// Contravariant
    pub contravariant: bool,
}

impl TypeVarInfo {
    /// Create a new TypeVar.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bound: None,
            constraints: Vec::new(),
            covariant: false,
            contravariant: false,
        }
    }

    /// Set bound.
    pub fn with_bound(mut self, bound: Type) -> Self {
        self.bound = Some(bound);
        self
    }

    /// Add constraint.
    pub fn with_constraint(mut self, constraint: Type) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Set covariant.
    pub fn covariant(mut self) -> Self {
        self.covariant = true;
        self
    }

    /// Set contravariant.
    pub fn contravariant(mut self) -> Self {
        self.contravariant = true;
        self
    }
}

/// Protocol definition for structural typing.
#[derive(Debug, Clone)]
pub struct ProtocolDef {
    /// Protocol name
    pub name: String,
    /// Required methods
    pub methods: HashMap<String, MethodSignature>,
    /// Required attributes
    pub attributes: HashMap<String, Type>,
    /// Parent protocols
    pub parents: Vec<String>,
}

/// Method signature in a protocol.
#[derive(Debug, Clone)]
pub struct MethodSignature {
    /// Method name
    pub name: String,
    /// Parameter types
    pub params: Vec<(String, Type)>,
    /// Return type
    pub return_type: Type,
    /// Is async
    pub is_async: bool,
}

/// Key for generic instantiation cache.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GenericKey {
    /// Base generic type
    pub base: String,
    /// Type arguments
    pub args: Vec<String>,
}
