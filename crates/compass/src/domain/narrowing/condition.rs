//! Type narrowing based on control flow analysis
//!
//! This module handles type narrowing for:
//! - isinstance() checks
//! - None checks (is None, is not None)
//! - Truthiness checks
//! - Type guards

use crate::domain::type_system::ty::Type;

/// Represents a condition that can narrow types
#[derive(Debug, Clone)]
pub enum NarrowingCondition {
    /// isinstance(x, T) or isinstance(x, (T1, T2))
    IsInstance { var_name: String, types: Vec<Type> },
    /// x is None
    IsNone { var_name: String },
    /// x is not None
    IsNotNone { var_name: String },
    /// x (truthiness check)
    Truthy { var_name: String },
    /// not x (falsiness check)
    Falsy { var_name: String },
    /// x == literal
    Equals { var_name: String, value: Type },
    /// x != literal
    NotEquals { var_name: String, value: Type },
    /// Compound: cond1 and cond2
    And(Box<NarrowingCondition>, Box<NarrowingCondition>),
    /// Compound: cond1 or cond2
    Or(Box<NarrowingCondition>, Box<NarrowingCondition>),
    /// Negation: not cond
    Not(Box<NarrowingCondition>),
    /// hasattr(x, "attr") - checks if object has an attribute
    HasAttr { var_name: String, attr_name: String },
    /// callable(x) - checks if object is callable
    IsCallable { var_name: String },
    /// not callable(x)
    NotCallable { var_name: String },
    /// TypeGuard function call (PEP 647)
    /// Narrows type only in positive branch
    TypeGuard {
        var_name: String,
        narrowed_type: Type,
    },
    /// TypeIs function call (PEP 742)
    /// Narrows type in both positive and negative branches
    TypeIs {
        var_name: String,
        narrowed_type: Type,
    },
    /// type(x) is T
    TypeCheck { var_name: String, target_type: Type },
    /// type(x) is not T
    NotTypeCheck { var_name: String, target_type: Type },
    /// Unknown/unanalyzable condition
    Unknown,
}

/// Negate a narrowing condition (public helper)
pub fn negate_condition(condition: &NarrowingCondition) -> NarrowingCondition {
    match condition {
        NarrowingCondition::IsNone { var_name } => NarrowingCondition::IsNotNone {
            var_name: var_name.clone(),
        },
        NarrowingCondition::IsNotNone { var_name } => NarrowingCondition::IsNone {
            var_name: var_name.clone(),
        },
        NarrowingCondition::Truthy { var_name } => NarrowingCondition::Falsy {
            var_name: var_name.clone(),
        },
        NarrowingCondition::Falsy { var_name } => NarrowingCondition::Truthy {
            var_name: var_name.clone(),
        },
        NarrowingCondition::And(left, right) => {
            // not (A and B) = (not A) or (not B)
            NarrowingCondition::Or(
                Box::new(negate_condition(left)),
                Box::new(negate_condition(right)),
            )
        }
        NarrowingCondition::Or(left, right) => {
            // not (A or B) = (not A) and (not B)
            NarrowingCondition::And(
                Box::new(negate_condition(left)),
                Box::new(negate_condition(right)),
            )
        }
        NarrowingCondition::Not(inner) => {
            // not (not x) = x
            (**inner).clone()
        }
        NarrowingCondition::IsCallable { var_name } => NarrowingCondition::NotCallable {
            var_name: var_name.clone(),
        },
        NarrowingCondition::NotCallable { var_name } => NarrowingCondition::IsCallable {
            var_name: var_name.clone(),
        },
        // TypeGuard doesn't narrow in negative branch (PEP 647)
        NarrowingCondition::TypeGuard { .. } => NarrowingCondition::Unknown,
        // TypeIs narrows in both branches (PEP 742)
        NarrowingCondition::TypeIs {
            var_name,
            narrowed_type,
        } => NarrowingCondition::Not(Box::new(NarrowingCondition::TypeIs {
            var_name: var_name.clone(),
            narrowed_type: narrowed_type.clone(),
        })),
        NarrowingCondition::TypeCheck {
            var_name,
            target_type,
        } => NarrowingCondition::NotTypeCheck {
            var_name: var_name.clone(),
            target_type: target_type.clone(),
        },
        NarrowingCondition::NotTypeCheck {
            var_name,
            target_type,
        } => NarrowingCondition::TypeCheck {
            var_name: var_name.clone(),
            target_type: target_type.clone(),
        },
        other => other.clone(),
    }
}
