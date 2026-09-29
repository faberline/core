use serde::{Deserialize, Serialize};

use crate::domain::type_system::ty::{LiteralValue, Param, ParamKind, Type};

/// Owned type information that can be serialized
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeInfo {
    /// Unknown type (not yet inferred)
    Unknown,
    /// Any type
    Any,
    /// None type
    None,
    /// Boolean type
    Bool,
    /// Integer type
    Int,
    /// Float type
    Float,
    /// String type
    Str,
    /// Bytes type
    Bytes,
    /// List type with element type
    List(Box<TypeInfo>),
    /// Set type with element type
    Set(Box<TypeInfo>),
    /// Dict type with key and value types
    Dict(Box<TypeInfo>, Box<TypeInfo>),
    /// Tuple type with element types
    Tuple(Vec<TypeInfo>),
    /// Optional type (T | None)
    Optional(Box<TypeInfo>),
    /// Union of types
    Union(Vec<TypeInfo>),
    /// Callable type
    Callable {
        params: Vec<ParamInfo>,
        return_type: Box<TypeInfo>,
    },
    /// Instance of a class
    Instance {
        name: String,
        module: Option<String>,
        type_args: Vec<TypeInfo>,
    },
    /// Literal type
    Literal(LiteralInfo),
    /// Error type (for error recovery)
    Error,
}

impl TypeInfo {
    /// Convert from the type checker's Type to owned TypeInfo
    pub fn from_type(ty: &Type) -> Self {
        match ty {
            Type::Unknown => TypeInfo::Unknown,
            Type::Any => TypeInfo::Any,
            Type::None => TypeInfo::None,
            Type::Bool => TypeInfo::Bool,
            Type::Int => TypeInfo::Int,
            Type::Float => TypeInfo::Float,
            Type::Str => TypeInfo::Str,
            Type::Bytes => TypeInfo::Bytes,
            Type::List(inner) => TypeInfo::List(Box::new(TypeInfo::from_type(inner))),
            Type::Set(inner) => TypeInfo::Set(Box::new(TypeInfo::from_type(inner))),
            Type::Dict(key, value) => TypeInfo::Dict(
                Box::new(TypeInfo::from_type(key)),
                Box::new(TypeInfo::from_type(value)),
            ),
            Type::Tuple(elems) => TypeInfo::Tuple(elems.iter().map(TypeInfo::from_type).collect()),
            Type::Optional(inner) => TypeInfo::Optional(Box::new(TypeInfo::from_type(inner))),
            Type::Union(types) => TypeInfo::Union(types.iter().map(TypeInfo::from_type).collect()),
            Type::Callable { params, ret } => TypeInfo::Callable {
                params: params.iter().map(ParamInfo::from_param).collect(),
                return_type: Box::new(TypeInfo::from_type(ret)),
            },
            Type::Instance {
                name,
                module,
                type_args,
            } => TypeInfo::Instance {
                name: name.clone(),
                module: module.clone(),
                type_args: type_args.iter().map(TypeInfo::from_type).collect(),
            },
            Type::Literal(lit) => TypeInfo::Literal(LiteralInfo::from_literal(lit)),
            Type::Error => TypeInfo::Error,
            // Handle other types by converting to string representation
            _ => TypeInfo::Instance {
                name: format!("{}", ty),
                module: None,
                type_args: vec![],
            },
        }
    }

    /// Display the type as a string
    pub fn display(&self) -> String {
        match self {
            TypeInfo::Unknown => "Unknown".to_string(),
            TypeInfo::Any => "Any".to_string(),
            TypeInfo::None => "None".to_string(),
            TypeInfo::Bool => "bool".to_string(),
            TypeInfo::Int => "int".to_string(),
            TypeInfo::Float => "float".to_string(),
            TypeInfo::Str => "str".to_string(),
            TypeInfo::Bytes => "bytes".to_string(),
            TypeInfo::List(inner) => format!("list[{}]", inner.display()),
            TypeInfo::Set(inner) => format!("set[{}]", inner.display()),
            TypeInfo::Dict(key, value) => format!("dict[{}, {}]", key.display(), value.display()),
            TypeInfo::Tuple(elems) => {
                let inner = elems
                    .iter()
                    .map(|t| t.display())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("tuple[{}]", inner)
            }
            TypeInfo::Optional(inner) => format!("{} | None", inner.display()),
            TypeInfo::Union(types) => types
                .iter()
                .map(|t| t.display())
                .collect::<Vec<_>>()
                .join(" | "),
            TypeInfo::Callable {
                params,
                return_type,
            } => {
                let param_str = params
                    .iter()
                    .map(|p| {
                        if p.name.is_empty() {
                            p.type_info.display()
                        } else {
                            format!("{}: {}", p.name, p.type_info.display())
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({}) -> {}", param_str, return_type.display())
            }
            TypeInfo::Instance {
                name, type_args, ..
            } => {
                if type_args.is_empty() {
                    name.clone()
                } else {
                    let args = type_args
                        .iter()
                        .map(|t| t.display())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}[{}]", name, args)
                }
            }
            TypeInfo::Literal(lit) => lit.display(),
            TypeInfo::Error => "<error>".to_string(),
        }
    }

    /// Check if this is an unknown type
    pub fn is_unknown(&self) -> bool {
        matches!(self, TypeInfo::Unknown)
    }
}

/// Parameter information for callable types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamInfo {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub type_info: TypeInfo,
    /// Whether the parameter has a default value
    pub has_default: bool,
    /// Whether this is a *args parameter
    pub is_variadic: bool,
    /// Whether this is a **kwargs parameter
    pub is_keyword: bool,
}

impl ParamInfo {
    /// Convert from the type checker's Param to owned ParamInfo
    pub fn from_param(param: &Param) -> Self {
        Self {
            name: param.name.clone(),
            type_info: TypeInfo::from_type(&param.ty),
            has_default: param.has_default,
            is_variadic: matches!(param.kind, ParamKind::VarPositional),
            is_keyword: matches!(param.kind, ParamKind::VarKeyword),
        }
    }
}

/// Literal value information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiteralInfo {
    Int(i64),
    Float(String), // Store as string to preserve exact representation
    Str(String),
    Bool(bool),
    None,
}

impl LiteralInfo {
    /// Convert from the type checker's LiteralValue
    pub fn from_literal(lit: &LiteralValue) -> Self {
        match lit {
            LiteralValue::Int(i) => LiteralInfo::Int(*i),
            LiteralValue::Float(f) => LiteralInfo::Float(f.to_string()),
            LiteralValue::Str(s) => LiteralInfo::Str(s.clone()),
            LiteralValue::Bool(b) => LiteralInfo::Bool(*b),
            LiteralValue::None => LiteralInfo::None,
        }
    }

    /// Display the literal value
    pub fn display(&self) -> String {
        match self {
            LiteralInfo::Int(i) => format!("Literal[{}]", i),
            LiteralInfo::Float(f) => format!("Literal[{}]", f),
            LiteralInfo::Str(s) => format!("Literal[\"{}\"]", s),
            LiteralInfo::Bool(b) => format!("Literal[{}]", b),
            LiteralInfo::None => "Literal[None]".to_string(),
        }
    }
}
