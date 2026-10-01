/// Type information (basic)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeInfo {
    /// Primitive types (int, str, bool, etc.)
    Primitive(String),
    /// List/Array type
    List(Box<TypeInfo>),
    /// Dict/Map type
    Dict(Box<TypeInfo>, Box<TypeInfo>),
    /// Optional type
    Optional(Box<TypeInfo>),
    /// Union type
    Union(Vec<TypeInfo>),
    /// Callable/Function type
    Callable {
        params: Vec<TypeInfo>,
        ret: Box<TypeInfo>,
    },
    /// Named type (class, interface, etc.)
    Named(String),
    /// Generic type with parameters
    Generic(String, Vec<TypeInfo>),
    /// Reference type (Rust &T, &mut T)
    Reference(Box<TypeInfo>),
    /// Unknown type
    Unknown,
    /// Any type
    Any,
    /// Error type - placeholder for unresolved expressions in error contexts
    /// Used to prevent cascading errors when the parser encounters syntax errors
    Error,
}

impl TypeInfo {
    /// Format type for display
    pub fn display(&self) -> String {
        match self {
            TypeInfo::Primitive(name) => name.clone(),
            TypeInfo::List(inner) => format!("list[{}]", inner.display()),
            TypeInfo::Dict(key, value) => format!("dict[{}, {}]", key.display(), value.display()),
            TypeInfo::Optional(inner) => format!("{}?", inner.display()),
            TypeInfo::Union(types) => types
                .iter()
                .map(|t| t.display())
                .collect::<Vec<_>>()
                .join(" | "),
            TypeInfo::Callable { params, ret } => {
                let params_str = params
                    .iter()
                    .map(|t| t.display())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({}) -> {}", params_str, ret.display())
            }
            TypeInfo::Named(name) => name.clone(),
            TypeInfo::Generic(name, args) => {
                let args_str = args
                    .iter()
                    .map(|t| t.display())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}<{}>", name, args_str)
            }
            TypeInfo::Reference(inner) => format!("&{}", inner.display()),
            TypeInfo::Unknown => "unknown".to_string(),
            TypeInfo::Any => "any".to_string(),
            TypeInfo::Error => "<error>".to_string(),
        }
    }

    /// Parse from Python type annotation string
    pub fn from_python_annotation(annotation: &str) -> Self {
        let annotation = annotation.trim();

        // Handle Optional
        if annotation.starts_with("Optional[") && annotation.ends_with(']') {
            let inner = &annotation[9..annotation.len() - 1];
            return TypeInfo::Optional(Box::new(Self::from_python_annotation(inner)));
        }

        // Handle List
        if annotation.starts_with("List[") && annotation.ends_with(']') {
            let inner = &annotation[5..annotation.len() - 1];
            return TypeInfo::List(Box::new(Self::from_python_annotation(inner)));
        }
        if annotation.starts_with("list[") && annotation.ends_with(']') {
            let inner = &annotation[5..annotation.len() - 1];
            return TypeInfo::List(Box::new(Self::from_python_annotation(inner)));
        }

        // Handle Dict
        if (annotation.starts_with("Dict[") || annotation.starts_with("dict["))
            && annotation.ends_with(']')
        {
            let inner = &annotation[5..annotation.len() - 1];
            if let Some((key, value)) = inner.split_once(',') {
                return TypeInfo::Dict(
                    Box::new(Self::from_python_annotation(key.trim())),
                    Box::new(Self::from_python_annotation(value.trim())),
                );
            }
        }

        // Handle Union with |
        if annotation.contains(" | ") {
            let types: Vec<_> = annotation
                .split(" | ")
                .map(|t| Self::from_python_annotation(t.trim()))
                .collect();
            return TypeInfo::Union(types);
        }

        // Handle primitives
        match annotation {
            "int" => TypeInfo::Primitive("int".to_string()),
            "str" => TypeInfo::Primitive("str".to_string()),
            "bool" => TypeInfo::Primitive("bool".to_string()),
            "float" => TypeInfo::Primitive("float".to_string()),
            "None" => TypeInfo::Primitive("None".to_string()),
            "Any" => TypeInfo::Any,
            _ => TypeInfo::Named(annotation.to_string()),
        }
    }

    /// Parse from Rust type annotation string
    pub fn from_rust_type(type_str: &str) -> Self {
        let type_str = type_str.trim();

        if type_str.is_empty() {
            return TypeInfo::Unknown;
        }

        // Handle references
        if let Some(inner) = type_str.strip_prefix("&mut ") {
            return TypeInfo::Reference(Box::new(Self::from_rust_type(inner)));
        }
        if let Some(inner) = type_str.strip_prefix('&') {
            return TypeInfo::Reference(Box::new(Self::from_rust_type(inner)));
        }

        // Handle Option<T>
        if type_str.starts_with("Option<") && type_str.ends_with('>') {
            let inner = &type_str[7..type_str.len() - 1];
            return TypeInfo::Optional(Box::new(Self::from_rust_type(inner)));
        }

        // Handle Vec<T>
        if type_str.starts_with("Vec<") && type_str.ends_with('>') {
            let inner = &type_str[4..type_str.len() - 1];
            return TypeInfo::List(Box::new(Self::from_rust_type(inner)));
        }

        // Handle HashMap<K, V>
        if type_str.starts_with("HashMap<") && type_str.ends_with('>') {
            let inner = &type_str[8..type_str.len() - 1];
            if let Some((key, value)) = inner.split_once(',') {
                return TypeInfo::Dict(
                    Box::new(Self::from_rust_type(key.trim())),
                    Box::new(Self::from_rust_type(value.trim())),
                );
            }
        }

        // Handle Result<T, E> and other generics with <>
        if let Some(lt_pos) = type_str.find('<') {
            if type_str.ends_with('>') {
                let name = &type_str[..lt_pos];
                let inner = &type_str[lt_pos + 1..type_str.len() - 1];
                let args: Vec<TypeInfo> = inner
                    .split(',')
                    .map(|t| Self::from_rust_type(t.trim()))
                    .collect();
                return TypeInfo::Generic(name.to_string(), args);
            }
        }

        // Handle Rust primitives
        match type_str {
            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
            | "u128" | "usize" | "f32" | "f64" | "bool" | "char" | "str" | "()" => {
                TypeInfo::Primitive(type_str.to_string())
            }
            "String" => TypeInfo::Named("String".to_string()),
            _ => TypeInfo::Named(type_str.to_string()),
        }
    }
}
