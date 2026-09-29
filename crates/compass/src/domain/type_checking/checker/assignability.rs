use crate::domain::type_checking::checker::TypeChecker;
use crate::domain::type_system::ty::{LiteralValue, Type};

impl<'a> TypeChecker<'a> {
    /// Check if source type is assignable to target type
    pub(super) fn is_assignable(&self, target: &Type, source: &Type) -> bool {
        // Any accepts anything
        if target.is_any() || source.is_any() {
            return true;
        }

        // Unknown is compatible with anything (not yet inferred)
        if target.is_unknown() || source.is_unknown() {
            return true;
        }

        // Error type is compatible with anything (avoid cascading errors)
        if target.is_error() || source.is_error() {
            return true;
        }

        // Same type
        if target == source {
            return true;
        }

        match (target, source) {
            // Optional accepts None
            (Type::Optional(_), Type::None) => true,
            (Type::Optional(inner), other) => self.is_assignable(inner, other),

            // Union accepts any member
            (Type::Union(types), src) => types.iter().any(|t| self.is_assignable(t, src)),
            (target, Type::Union(types)) => types.iter().all(|t| self.is_assignable(target, t)),

            // Float accepts Int
            (Type::Float, Type::Int) => true,

            // List covariance
            (Type::List(a), Type::List(b)) => self.is_assignable(a, b),

            // Dict invariance (simplified - should be covariant in value)
            (Type::Dict(k1, v1), Type::Dict(k2, v2)) => {
                self.is_assignable(k1, k2) && self.is_assignable(v1, v2)
            }

            // Tuple structural compatibility
            (Type::Tuple(a), Type::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(t1, t2)| self.is_assignable(t1, t2))
            }

            // Class subtyping with inheritance and variance checking
            (
                Type::Instance {
                    name: n1,
                    type_args: args1,
                    ..
                },
                Type::Instance {
                    name: n2,
                    type_args: args2,
                    ..
                },
            ) => {
                // Different class names - check subtyping
                if n1 != n2 {
                    return self.inferencer.is_subclass(n2, n1);
                }

                // Same class name - check type arguments with variance
                if args1.is_empty() && args2.is_empty() {
                    return true; // Non-generic or unparameterized
                }

                // If one is generic and one is not, allow (gradual typing)
                if args1.is_empty() || args2.is_empty() {
                    return true;
                }

                // Check arity matches
                if args1.len() != args2.len() {
                    return false;
                }

                // Check each type argument with variance
                self.check_type_args_with_variance(n1, args1, args2)
            }

            // Protocol structural subtyping
            // A type is assignable to a Protocol if it has all required members
            (Type::Protocol { members, .. }, Type::Instance { name, .. }) => {
                // Check that the source type has all required protocol members
                members.iter().all(|(member_name, member_ty)| {
                    if let Some(attr_ty) =
                        self.inferencer.get_attribute_recursive(name, member_name)
                    {
                        self.is_assignable(member_ty, &attr_ty)
                    } else {
                        false
                    }
                })
            }

            // Callable to Protocol with __call__
            (Type::Protocol { members, .. }, Type::Callable { .. }) => {
                // A Callable can match a Protocol if the Protocol only requires __call__
                members.len() == 1 && members.iter().any(|(name, _)| name == "__call__")
            }

            // Literal types are assignable to their base types
            (Type::Int, Type::Literal(LiteralValue::Int(_))) => true,
            (Type::Float, Type::Literal(LiteralValue::Float(_))) => true,
            (Type::Float, Type::Literal(LiteralValue::Int(_))) => true, // int literal -> float
            (Type::Str, Type::Literal(LiteralValue::Str(_))) => true,
            (Type::Bool, Type::Literal(LiteralValue::Bool(_))) => true,
            (Type::None, Type::Literal(LiteralValue::None)) => true,

            // Literal to Literal (same value)
            (Type::Literal(a), Type::Literal(b)) => a == b,

            // TypedDict structural subtyping
            // A dict is assignable to TypedDict if it has all required keys
            (Type::TypedDict { fields, .. }, Type::Dict(key_ty, val_ty)) => {
                // For a generic dict to be assignable to TypedDict,
                // key must be str and value must be compatible with all field types
                if !self.is_assignable(&Type::Str, key_ty) {
                    return false;
                }
                fields
                    .iter()
                    .all(|(_, field_ty, _)| self.is_assignable(field_ty, val_ty))
            }

            // TypedDict to TypedDict: all required fields must match
            (
                Type::TypedDict {
                    fields: target_fields,
                    ..
                },
                Type::TypedDict {
                    fields: source_fields,
                    ..
                },
            ) => {
                // Check all required target fields exist in source with compatible types
                target_fields.iter().all(|(name, ty, required)| {
                    if let Some((_, src_ty, _)) = source_fields.iter().find(|(n, _, _)| n == name) {
                        self.is_assignable(ty, src_ty)
                    } else {
                        !required // Missing field is ok only if not required
                    }
                })
            }

            // TypedDict is a subtype of Dict[str, Union[field_types]]
            (Type::Dict(key_ty, _), Type::TypedDict { .. }) => {
                self.is_assignable(key_ty, &Type::Str)
            }

            // PEP 591: Final[T] is assignable to T
            (target, Type::Final(inner)) => self.is_assignable(target, inner),
            (Type::Final(inner), source) => self.is_assignable(inner, source),

            // PEP 593: Annotated[T, ...] is assignable to T
            (target, Type::Annotated { inner, .. }) => self.is_assignable(target, inner),
            (Type::Annotated { inner, .. }, source) => self.is_assignable(inner, source),

            // PEP 675: LiteralString is assignable to str
            (Type::Str, Type::LiteralString) => true,
            // String literals are assignable to LiteralString
            (Type::LiteralString, Type::Literal(LiteralValue::Str(_))) => true,
            // LiteralString is assignable to LiteralString
            (Type::LiteralString, Type::LiteralString) => true,

            // PEP 673: Self type - compare class names
            (
                Type::SelfType {
                    class_name: Some(n1),
                },
                Type::SelfType {
                    class_name: Some(n2),
                },
            ) => n1 == n2,
            (
                Type::SelfType {
                    class_name: Some(name),
                },
                Type::Instance {
                    name: inst_name, ..
                },
            ) => name == inst_name,
            (
                Type::Instance {
                    name: inst_name, ..
                },
                Type::SelfType {
                    class_name: Some(name),
                },
            ) => name == inst_name,
            // Unresolved Self is compatible with any Self
            (Type::SelfType { class_name: None }, Type::SelfType { .. }) => true,
            (Type::SelfType { .. }, Type::SelfType { class_name: None }) => true,

            // Overloaded functions - source can be any signature
            (Type::Callable { .. }, Type::Overloaded { signatures }) => {
                signatures.iter().any(|sig| self.is_assignable(target, sig))
            }
            // Overloaded to Callable - at least one signature must match
            (Type::Overloaded { signatures }, source) => {
                signatures.iter().any(|sig| self.is_assignable(sig, source))
            }

            _ => false,
        }
    }
}
