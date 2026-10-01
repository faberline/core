use std::collections::HashMap;

use crate::domain::narrowing::condition::NarrowingCondition;
use crate::domain::type_system::ty::Type;

/// Type narrower that tracks narrowed types in different branches
#[derive(Debug, Clone, Default)]
pub struct TypeNarrower {
    /// Stack of narrowing scopes (for nested if/else)
    scopes: Vec<NarrowingScope>,
}

/// A narrowing scope tracks type refinements in a specific branch
#[derive(Debug, Clone, Default)]
pub struct NarrowingScope {
    /// Narrowed types for variables in this scope
    pub narrowed: HashMap<String, Type>,
}

impl TypeNarrower {
    pub fn new() -> Self {
        Self { scopes: vec![] }
    }

    /// Push a new narrowing scope (entering an if/else branch)
    pub fn push_scope(&mut self) {
        self.scopes.push(NarrowingScope::default());
    }

    /// Pop the current narrowing scope (leaving a branch)
    pub fn pop_scope(&mut self) -> Option<NarrowingScope> {
        self.scopes.pop()
    }

    /// Apply a narrowing condition to the current scope
    pub fn apply_condition(
        &mut self,
        condition: &NarrowingCondition,
        original_types: &HashMap<String, Type>,
    ) {
        match condition {
            NarrowingCondition::IsInstance { var_name, types } => {
                // Narrow to the union of instance types
                let narrowed = if types.len() == 1 {
                    types[0].clone()
                } else {
                    Type::union(types.clone())
                };
                self.narrow_var(var_name, narrowed);
            }
            NarrowingCondition::IsNone { var_name } => {
                self.narrow_var(var_name, Type::None);
            }
            NarrowingCondition::IsNotNone { var_name } => {
                if let Some(original) = original_types.get(var_name) {
                    let narrowed = original.without_none();
                    self.narrow_var(var_name, narrowed);
                }
            }
            NarrowingCondition::Truthy { var_name } => {
                // Truthy narrows away None and False-like values
                if let Some(original) = original_types.get(var_name) {
                    let narrowed = original.without_none();
                    self.narrow_var(var_name, narrowed);
                }
            }
            NarrowingCondition::Falsy { var_name } => {
                // Falsy could mean None, False, 0, "", etc.
                // For Optional types, narrow to None
                if let Some(original) = original_types.get(var_name) {
                    if original.contains_none() {
                        self.narrow_var(var_name, Type::None);
                    }
                }
            }
            NarrowingCondition::Equals { var_name, value } => {
                self.narrow_var(var_name, value.clone());
            }
            NarrowingCondition::NotEquals { var_name, value } => {
                if let Some(original) = original_types.get(var_name) {
                    // If comparing against None, remove None from type
                    if matches!(value, Type::None) {
                        let narrowed = original.without_none();
                        self.narrow_var(var_name, narrowed);
                    }
                }
            }
            NarrowingCondition::And(left, right) => {
                self.apply_condition(left, original_types);
                self.apply_condition(right, original_types);
            }
            NarrowingCondition::Or(left, right) => {
                // For OR, we need to compute the union of both branches
                let mut left_narrower = self.clone();
                let mut right_narrower = self.clone();
                left_narrower.apply_condition(left, original_types);
                right_narrower.apply_condition(right, original_types);

                // Merge: take union of narrowed types
                // This is conservative - we keep the widest possible type
                // For now, just don't narrow on OR
            }
            NarrowingCondition::Not(inner) => {
                // Apply the negation of the condition
                let negated = Self::negate_condition_internal(inner);
                self.apply_condition(&negated, original_types);
            }
            NarrowingCondition::HasAttr {
                var_name,
                attr_name,
            } => {
                // hasattr() doesn't narrow the type itself, but we record it
                // so that attribute access on the variable is considered valid
                // For now, we just mark that we know it has this attribute
                if let Some(original) = original_types.get(var_name) {
                    // If the type has a structural attribute, narrow to that
                    // For now, keep the original type but mark narrowing happened
                    self.narrow_var(
                        var_name,
                        Type::Instance {
                            name: format!("HasAttr[{}, {}]", original, attr_name),
                            module: None,
                            type_args: vec![],
                        },
                    );
                }
            }
            NarrowingCondition::IsCallable { var_name } => {
                // callable() narrows to Callable type
                if let Some(_original) = original_types.get(var_name) {
                    self.narrow_var(
                        var_name,
                        Type::Callable {
                            params: vec![],
                            ret: Box::new(Type::Any),
                        },
                    );
                }
            }
            NarrowingCondition::NotCallable { var_name } => {
                // not callable() - we know it's not callable, but we can't really
                // narrow the type further without more context
                if let Some(original) = original_types.get(var_name) {
                    self.narrow_var(var_name, original.clone());
                }
            }
            NarrowingCondition::TypeGuard {
                var_name,
                narrowed_type,
            } => {
                // TypeGuard[T] narrows only in positive branch
                self.narrow_var(var_name, narrowed_type.clone());
            }
            NarrowingCondition::TypeIs {
                var_name,
                narrowed_type,
            } => {
                // TypeIs[T] also narrows in positive branch
                // The negative branch narrowing happens in negate_condition
                self.narrow_var(var_name, narrowed_type.clone());
            }
            NarrowingCondition::TypeCheck {
                var_name,
                target_type,
            } => {
                // type(x) is T - narrow to exact type
                self.narrow_var(var_name, target_type.clone());
            }
            NarrowingCondition::NotTypeCheck {
                var_name,
                target_type: _,
            } => {
                // type(x) is not T - can't narrow much without more context
                // Keep original type for now
                if let Some(original) = original_types.get(var_name) {
                    self.narrow_var(var_name, original.clone());
                }
            }
            NarrowingCondition::Unknown => {}
        }
    }

    /// Negate a condition (internal use)
    fn negate_condition_internal(condition: &NarrowingCondition) -> NarrowingCondition {
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
                    Box::new(Self::negate_condition_internal(left)),
                    Box::new(Self::negate_condition_internal(right)),
                )
            }
            NarrowingCondition::Or(left, right) => {
                // not (A or B) = (not A) and (not B)
                NarrowingCondition::And(
                    Box::new(Self::negate_condition_internal(left)),
                    Box::new(Self::negate_condition_internal(right)),
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
            // In negative branch, we exclude the narrowed type
            NarrowingCondition::TypeIs {
                var_name,
                narrowed_type,
            } => {
                // Negation creates an "exclude this type" condition
                // For now, we represent this as Unknown and handle specially
                NarrowingCondition::Not(Box::new(NarrowingCondition::TypeIs {
                    var_name: var_name.clone(),
                    narrowed_type: narrowed_type.clone(),
                }))
            }
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

    /// Set a narrowed type for a variable
    fn narrow_var(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.narrowed.insert(name.to_string(), ty);
        }
    }

    /// Get the narrowed type for a variable, if any
    pub fn get_narrowed(&self, name: &str) -> Option<&Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.narrowed.get(name) {
                return Some(ty);
            }
        }
        None
    }

    /// Compute the intersection of a type with the narrowed type
    pub fn narrow_type(&self, name: &str, original: &Type) -> Type {
        if let Some(narrowed) = self.get_narrowed(name) {
            // Return the more specific type
            narrowed.clone()
        } else {
            original.clone()
        }
    }
}

#[cfg(test)]
mod tests;
