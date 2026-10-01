//! Core type definitions for the Argus type system

mod constructors;
mod display;
mod generics;
mod predicates;

/// Unique identifier for type variables
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeVarId(pub usize);

/// Unique identifier for ParamSpec (PEP 612)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParamSpecId(pub usize);

/// Unique identifier for TypeVarTuple (PEP 646)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeVarTupleId(pub usize);

/// Variance of a TypeVar for generic types
///
/// Variance determines how subtyping works for generic types:
/// - Invariant: T[A] is only a subtype of T[B] if A == B (e.g., list in Python is invariant)
/// - Covariant: T[A] is a subtype of T[B] if A is a subtype of B (e.g., Sequence, return types)
/// - Contravariant: T[A] is a subtype of T[B] if B is a subtype of A (e.g., function params)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variance {
    /// Default: A and B must be exactly equal
    #[default]
    Invariant,
    /// T[A] <: T[B] if A <: B (e.g., Iterator[Cat] <: Iterator[Animal])
    Covariant,
    /// T[A] <: T[B] if B <: A (e.g., Callable[[Animal], None] <: Callable[[Cat], None])
    Contravariant,
}

impl Variance {
    /// Check if this variance allows subtyping in the given direction
    ///
    /// - Covariant: subtype of parameter means subtype of whole type
    /// - Contravariant: supertype of parameter means subtype of whole type
    /// - Invariant: exact match required
    pub fn allows_subtype(&self, is_subtype: bool, is_supertype: bool) -> bool {
        match self {
            Variance::Invariant => is_subtype && is_supertype, // Must be equal
            Variance::Covariant => is_subtype,
            Variance::Contravariant => is_supertype,
        }
    }
}

/// Parameter kind in function signatures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// Regular positional parameter
    Positional,
    /// Positional-only parameter (before /)
    PositionalOnly,
    /// Keyword-only parameter (after *)
    KeywordOnly,
    /// *args parameter
    VarPositional,
    /// **kwargs parameter
    VarKeyword,
}

/// Function parameter
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub has_default: bool,
    pub kind: ParamKind,
}

/// Literal value for Literal types
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    None,
}

/// The core Type enum representing all possible types
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // === Primitive types ===
    /// The bottom type (NoReturn in Python, never in TypeScript)
    Never,
    /// None / null / unit type
    None,
    /// Boolean
    Bool,
    /// Integer (Python int, TypeScript number)
    Int,
    /// Floating point (Python float, TypeScript number)
    Float,
    /// String
    Str,
    /// Bytes (Python only)
    Bytes,

    // === Container types ===
    /// List / Array
    List(Box<Type>),
    /// Dictionary / Map / Object
    Dict(Box<Type>, Box<Type>),
    /// Set
    Set(Box<Type>),
    /// Tuple with fixed element types
    Tuple(Vec<Type>),

    // === Composite types ===
    /// Optional type (T | None)
    Optional(Box<Type>),
    /// Union type (T | U | V)
    Union(Vec<Type>),
    /// Intersection type (T & U) - TypeScript only
    Intersection(Vec<Type>),

    // === Callable types ===
    /// Function / Callable type
    Callable { params: Vec<Param>, ret: Box<Type> },

    // === Class types ===
    /// Class / Interface / Struct instance
    Instance {
        name: String,
        module: Option<String>,
        type_args: Vec<Type>,
    },
    /// Class type itself (for type[T])
    ClassType {
        name: String,
        module: Option<String>,
    },

    // === Generic types ===
    /// Type variable (T, K, V, etc.)
    TypeVar {
        id: TypeVarId,
        name: String,
        bound: Option<Box<Type>>,
        constraints: Vec<Type>,
        /// Variance of this TypeVar (covariant, contravariant, or invariant)
        variance: Variance,
    },

    // === Protocol types ===
    /// Protocol type (structural subtyping)
    /// A type conforms to a Protocol if it has all required members
    Protocol {
        name: String,
        module: Option<String>,
        /// Required members (method/attribute name -> type)
        members: Vec<(String, Type)>,
    },

    // === TypedDict ===
    /// TypedDict type - dictionary with specific keys and types
    TypedDict {
        name: String,
        /// Fields with their types and whether they are required
        fields: Vec<(String, Type, bool)>, // (name, type, required)
        /// Whether all fields are required by default
        total: bool,
    },

    // === Special types ===
    /// Any type - disables type checking
    Any,
    /// Unknown type - not yet inferred
    Unknown,
    /// Literal type (Literal["foo"], Literal[42])
    Literal(LiteralValue),
    /// Self type (PEP 673) - references the enclosing class
    SelfType {
        /// The class this Self refers to (resolved during checking)
        class_name: Option<String>,
    },
    /// LiteralString type (PEP 675) - string known at compile time
    LiteralString,
    /// Final type (PEP 591) - value cannot be reassigned
    Final(Box<Type>),
    /// Annotated type (PEP 593) - type with runtime metadata
    Annotated {
        inner: Box<Type>,
        metadata: Vec<String>, // Simplified: just store as strings
    },

    // === Overloaded functions ===
    /// Overloaded function with multiple signatures
    Overloaded {
        signatures: Vec<Type>, // Each should be a Callable
    },

    // === ParamSpec (PEP 612) ===
    /// ParamSpec - captures function parameter types
    ParamSpec { id: ParamSpecId, name: String },
    /// Concatenate - prepend params to a ParamSpec
    Concatenate {
        params: Vec<Type>,
        param_spec: Box<Type>, // Should be a ParamSpec
    },

    // === TypeVarTuple (PEP 646) ===
    /// TypeVarTuple - variadic type variable
    TypeVarTuple { id: TypeVarTupleId, name: String },
    /// Unpacked TypeVarTuple (*Ts)
    Unpack(Box<Type>),

    // === Type Guards (PEP 647, 742) ===
    /// TypeGuard[T] - narrows type only in positive branch
    TypeGuard(Box<Type>),
    /// TypeIs[T] - narrows type in both positive and negative branches
    TypeIs(Box<Type>),

    // === Error type ===
    /// Type error placeholder (allows continued analysis)
    Error,
}

#[cfg(test)]
mod tests;
