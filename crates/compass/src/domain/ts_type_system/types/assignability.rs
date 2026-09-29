use crate::type_inference::Type;

/// Check if a type is assignable to another type (structural compatibility)
pub fn is_assignable_to(source: &Type, target: &Type) -> bool {
    match (source, target) {
        // Any accepts anything
        (_, Type::Any) | (Type::Any, _) => true,

        // Unknown is assignable only to unknown or any
        (Type::Unknown, Type::Unknown) => true,
        (Type::Unknown, _) => false,

        // Never is assignable to everything
        (Type::Never, _) => true,

        // Same primitive types
        (Type::None, Type::None)
        | (Type::Bool, Type::Bool)
        | (Type::Int, Type::Int)
        | (Type::Float, Type::Float)
        | (Type::Str, Type::Str)
        | (Type::Bytes, Type::Bytes) => true,

        // Number widening
        (Type::Int, Type::Float) => true,

        // Literal to base type
        (Type::Literal(crate::type_inference::LiteralValue::Str(_)), Type::Str) => true,
        (Type::Literal(crate::type_inference::LiteralValue::Int(_)), Type::Int) => true,
        (Type::Literal(crate::type_inference::LiteralValue::Int(_)), Type::Float) => true,
        (Type::Literal(crate::type_inference::LiteralValue::Float(_)), Type::Float) => true,
        (Type::Literal(crate::type_inference::LiteralValue::Bool(_)), Type::Bool) => true,

        // Same literals
        (Type::Literal(a), Type::Literal(b)) => a == b,

        // Container types
        (Type::List(a), Type::List(b)) => is_assignable_to(a, b),
        (Type::Set(a), Type::Set(b)) => is_assignable_to(a, b),
        (Type::Dict(k1, v1), Type::Dict(k2, v2)) => {
            is_assignable_to(k1, k2) && is_assignable_to(v1, v2)
        }

        // Tuple types
        (Type::Tuple(a), Type::Tuple(b)) if a.len() == b.len() => {
            a.iter().zip(b.iter()).all(|(s, t)| is_assignable_to(s, t))
        }

        // Union source: all members must be assignable
        (Type::Union(members), target) => members.iter().all(|m| is_assignable_to(m, target)),

        // Union target: source must be assignable to at least one
        (source, Type::Union(members)) => members.iter().any(|m| is_assignable_to(source, m)),

        // Intersection target: source must be assignable to all
        (source, Type::Intersection(members)) => {
            members.iter().all(|m| is_assignable_to(source, m))
        }

        // Intersection source: at least one member must be assignable
        (Type::Intersection(members), target) => {
            members.iter().any(|m| is_assignable_to(m, target))
        }

        // Optional types
        (Type::Optional(a), Type::Optional(b)) => is_assignable_to(a, b),
        (a, Type::Optional(b)) => is_assignable_to(a, b),

        // Instance types (nominal + structural)
        (
            Type::Instance {
                name: n1,
                type_args: a1,
                ..
            },
            Type::Instance {
                name: n2,
                type_args: a2,
                ..
            },
        ) => {
            n1 == n2
                && a1.len() == a2.len()
                && a1
                    .iter()
                    .zip(a2.iter())
                    .all(|(s, t)| is_assignable_to(s, t))
        }

        // Callable types (contravariant params, covariant return)
        (
            Type::Callable {
                params: p1,
                ret: r1,
            },
            Type::Callable {
                params: p2,
                ret: r2,
            },
        ) => {
            // Source can have fewer required params
            if p1.len() > p2.len() {
                return false;
            }
            // Contravariant params
            let params_ok = p1
                .iter()
                .zip(p2.iter())
                .all(|(s, t)| is_assignable_to(&t.ty, &s.ty));
            // Covariant return
            params_ok && is_assignable_to(r1, r2)
        }

        // Protocol/interface structural matching
        (source, Type::Protocol { members, .. }) => {
            // Check if source has all required members
            check_protocol_conformance(source, members)
        }

        _ => false,
    }
}

/// Check if a type conforms to a protocol (structural subtyping)
fn check_protocol_conformance(ty: &Type, members: &[(String, Type)]) -> bool {
    match ty {
        Type::Instance { .. } => {
            // Would need to look up class info - simplified here
            true // Placeholder: assume conformance for instances
        }
        Type::Protocol {
            members: source_members,
            ..
        } => {
            // All target members must exist in source with compatible types
            members.iter().all(|(name, target_ty)| {
                source_members.iter().any(|(src_name, src_ty)| {
                    src_name == name && is_assignable_to(src_ty, target_ty)
                })
            })
        }
        _ => false,
    }
}
