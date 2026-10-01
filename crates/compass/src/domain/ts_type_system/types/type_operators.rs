use std::collections::HashMap;

use crate::domain::ts_type_system::types::is_assignable_to;
use crate::type_inference::{Type, TypeVarId};

/// TypeScript mapped type: { [K in Keys]: ValueType }
#[derive(Debug, Clone, PartialEq)]
pub struct TsMappedType {
    /// Key variable name
    pub key_var: String,
    /// Keys to iterate over (usually a union of string literals)
    pub keys: Type,
    /// Value type (may reference key_var)
    pub value_type: Type,
    /// Optional modifier: +? / -? / ?
    pub optional_modifier: Option<MappedTypeModifier>,
    /// Readonly modifier: +readonly / -readonly / readonly
    pub readonly_modifier: Option<MappedTypeModifier>,
}

/// Modifier for mapped types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedTypeModifier {
    /// Add the modifier
    Add,
    /// Remove the modifier
    Remove,
    /// Preserve (or add) the modifier
    Preserve,
}

/// TypeScript conditional type: T extends U ? X : Y
#[derive(Debug, Clone, PartialEq)]
pub struct TsConditionalType {
    /// Check type (T)
    pub check_type: Box<Type>,
    /// Extends type (U)
    pub extends_type: Box<Type>,
    /// True branch type (X)
    pub true_type: Box<Type>,
    /// False branch type (Y)
    pub false_type: Box<Type>,
}

impl TsConditionalType {
    /// Evaluate the conditional type with concrete substitutions
    pub fn evaluate(&self, subs: &HashMap<TypeVarId, Type>) -> Type {
        let check = self.check_type.substitute(subs);
        let extends = self.extends_type.substitute(subs);

        // Simple structural check
        if is_assignable_to(&check, &extends) {
            self.true_type.substitute(subs)
        } else {
            self.false_type.substitute(subs)
        }
    }
}

/// TypeScript template literal type: `prefix${T}suffix`
#[derive(Debug, Clone, PartialEq)]
pub struct TsTemplateLiteralType {
    /// Parts of the template
    pub parts: Vec<TemplatePart>,
}

/// Part of a template literal type
#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    /// Literal string segment
    Literal(String),
    /// Type placeholder (usually string, number, or type variable)
    Placeholder(Type),
}

impl TsTemplateLiteralType {
    /// Create a simple template with prefix and suffix
    pub fn new(prefix: &str, inner: Type, suffix: &str) -> Self {
        let mut parts = Vec::new();
        if !prefix.is_empty() {
            parts.push(TemplatePart::Literal(prefix.to_string()));
        }
        parts.push(TemplatePart::Placeholder(inner));
        if !suffix.is_empty() {
            parts.push(TemplatePart::Literal(suffix.to_string()));
        }
        Self { parts }
    }

    /// Evaluate template with concrete types to produce literal string types
    pub fn evaluate(&self, subs: &HashMap<TypeVarId, Type>) -> Type {
        // If all placeholders resolve to literals, produce literal type
        let mut result = String::new();
        let mut all_literal = true;

        for part in &self.parts {
            match part {
                TemplatePart::Literal(s) => result.push_str(s),
                TemplatePart::Placeholder(ty) => {
                    let resolved = ty.substitute(subs);
                    if let Type::Literal(crate::type_inference::LiteralValue::Str(s)) = resolved {
                        result.push_str(&s);
                    } else {
                        all_literal = false;
                        break;
                    }
                }
            }
        }

        if all_literal {
            Type::Literal(crate::type_inference::LiteralValue::Str(result))
        } else {
            Type::Str
        }
    }
}
