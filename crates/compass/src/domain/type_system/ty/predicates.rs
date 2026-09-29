use crate::domain::type_system::ty::{LiteralValue, Type};

impl Type {
    /// Check if this type is a subtype of Any
    pub fn is_any(&self) -> bool {
        matches!(self, Type::Any)
    }

    /// Check if this type is Unknown
    pub fn is_unknown(&self) -> bool {
        matches!(self, Type::Unknown)
    }

    /// Check if this type is an error
    pub fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }

    /// Check if this type contains None
    pub fn contains_none(&self) -> bool {
        match self {
            Type::None => true,
            Type::Optional(_) => true,
            Type::Union(types) => types.iter().any(|t| t.contains_none()),
            _ => false,
        }
    }

    /// Remove None from this type
    pub fn without_none(&self) -> Type {
        match self {
            Type::None => Type::Never,
            Type::Optional(inner) => (**inner).clone(),
            Type::Union(types) => {
                let filtered: Vec<_> = types
                    .iter()
                    .filter(|t| !matches!(t, Type::None))
                    .cloned()
                    .collect();
                Type::union(filtered)
            }
            other => other.clone(),
        }
    }

    /// Unwrap Final to get inner type
    pub fn unwrap_final(&self) -> &Type {
        match self {
            Type::Final(inner) => inner,
            other => other,
        }
    }

    /// Unwrap Annotated to get inner type
    pub fn unwrap_annotated(&self) -> &Type {
        match self {
            Type::Annotated { inner, .. } => inner,
            other => other,
        }
    }

    /// Check if this is a Final type
    pub fn is_final(&self) -> bool {
        matches!(self, Type::Final(_))
    }

    /// Check if this is a LiteralString
    pub fn is_literal_string(&self) -> bool {
        matches!(
            self,
            Type::LiteralString | Type::Literal(LiteralValue::Str(_))
        )
    }

    /// Check if this is a TypeGuard
    pub fn is_type_guard(&self) -> bool {
        matches!(self, Type::TypeGuard(_))
    }

    /// Check if this is a TypeIs
    pub fn is_type_is(&self) -> bool {
        matches!(self, Type::TypeIs(_))
    }

    /// Get the narrowed type from TypeGuard or TypeIs
    pub fn get_guard_type(&self) -> Option<&Type> {
        match self {
            Type::TypeGuard(inner) | Type::TypeIs(inner) => Some(inner),
            _ => None,
        }
    }
}
