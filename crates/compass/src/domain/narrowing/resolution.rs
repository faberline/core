use std::collections::HashMap;

use crate::domain::type_system::ty::{Param, Type, TypeVarId};

// ============================================================================
// Generic TypeVar resolution (R2.2)
// ============================================================================

/// Resolve TypeVar bindings from function call arguments
///
/// Given a generic function signature like `def get(items: list[T]) -> T`
/// and a call `get([1, 2, 3])`, this resolves `T = int`.
///
/// Returns a map of TypeVarId -> concrete Type.
pub fn resolve_typevar_bindings(
    param_types: &[Type],
    arg_types: &[Type],
) -> HashMap<TypeVarId, Type> {
    let mut bindings = HashMap::new();

    for (param_ty, arg_ty) in param_types.iter().zip(arg_types.iter()) {
        param_ty.unify(arg_ty, &mut bindings);
    }

    bindings
}

/// Apply TypeVar bindings to a return type to get the concrete return type
pub fn apply_typevar_bindings(return_type: &Type, bindings: &HashMap<TypeVarId, Type>) -> Type {
    return_type.substitute(bindings)
}

// ============================================================================
// Protocol structural typing (R2.3)
// ============================================================================

/// Check if a type structurally satisfies a Protocol
///
/// A type satisfies a Protocol if it has all required members (methods/attributes)
/// with compatible types. This implements duck typing via typing.Protocol.
pub fn check_protocol_satisfaction(
    ty: &Type,
    protocol_members: &[(String, Type)],
    type_registry: &HashMap<String, Vec<(String, Type)>>,
) -> ProtocolCheckResult {
    let members = get_type_members(ty, type_registry);

    let mut missing = Vec::new();
    let mut incompatible = Vec::new();

    for (member_name, member_type) in protocol_members {
        if let Some(&actual_type) = members.get(member_name.as_str()) {
            // Check type compatibility (simplified: Any is always compatible)
            if !types_compatible(actual_type, member_type) {
                incompatible.push(ProtocolMemberError {
                    member: member_name.clone(),
                    expected: member_type.clone(),
                    actual: actual_type.clone(),
                });
            }
        } else {
            missing.push(member_name.clone());
        }
    }

    ProtocolCheckResult {
        missing,
        incompatible,
    }
}

/// Result of a protocol satisfaction check
#[derive(Debug, Clone)]
pub struct ProtocolCheckResult {
    pub missing: Vec<String>,
    pub incompatible: Vec<ProtocolMemberError>,
}

impl ProtocolCheckResult {
    pub fn is_satisfied(&self) -> bool {
        self.missing.is_empty() && self.incompatible.is_empty()
    }
}

/// An incompatible member error
#[derive(Debug, Clone)]
pub struct ProtocolMemberError {
    pub member: String,
    pub expected: Type,
    pub actual: Type,
}

/// Get the members of a type (methods and attributes)
fn get_type_members<'a>(
    ty: &'a Type,
    registry: &'a HashMap<String, Vec<(String, Type)>>,
) -> HashMap<&'a str, &'a Type> {
    match ty {
        Type::Instance { name, .. } => {
            if let Some(members) = registry.get(name.as_str()) {
                return members.iter().map(|(k, v)| (k.as_str(), v)).collect();
            }
        }
        Type::Protocol { members, .. } => {
            return members.iter().map(|(k, v)| (k.as_str(), v)).collect();
        }
        _ => {}
    }
    HashMap::new()
}

/// Check if two types are compatible (simplified structural check)
fn types_compatible(actual: &Type, expected: &Type) -> bool {
    match (actual, expected) {
        (_, Type::Any) => true,
        (Type::Any, _) => true,
        (Type::Unknown, _) => true,
        (a, b) => a == b,
    }
}

// ============================================================================
// @overload resolution (R2.4)
// ============================================================================

/// Resolve the correct overload for a function call
///
/// Given an Overloaded type and the argument types, selects the matching
/// overload signature using the same algorithm as Pyright:
/// 1. Try each overload in order
/// 2. Return the first one where all argument types are compatible with params
/// 3. Fall back to the last overload if none match (error recovery)
pub fn resolve_overload(overloaded: &Type, arg_types: &[Type]) -> Option<Type> {
    let signatures = match overloaded {
        Type::Overloaded { signatures } => signatures,
        // Not overloaded — just return as-is
        other => return Some(other.clone()),
    };

    for sig in signatures {
        if let Type::Callable { params, ret } = sig {
            if overload_matches(params, arg_types) {
                return Some((**ret).clone());
            }
        }
    }

    // Fall back to last signature's return type for error recovery
    signatures.last().and_then(|sig| {
        if let Type::Callable { ret, .. } = sig {
            Some((**ret).clone())
        } else {
            None
        }
    })
}

/// Check if argument types match an overload's parameter types
fn overload_matches(params: &[Param], arg_types: &[Type]) -> bool {
    use crate::domain::type_system::ty::ParamKind;

    // Count required positional params
    let required_count = params
        .iter()
        .filter(|p| {
            !p.has_default && matches!(p.kind, ParamKind::Positional | ParamKind::PositionalOnly)
        })
        .count();

    let max_count = params
        .iter()
        .filter(|p| {
            matches!(
                p.kind,
                ParamKind::Positional | ParamKind::PositionalOnly | ParamKind::KeywordOnly
            )
        })
        .count();

    // Check arg count
    if arg_types.len() < required_count || arg_types.len() > max_count {
        // Check if there's a *args param
        let has_var_positional = params
            .iter()
            .any(|p| matches!(p.kind, ParamKind::VarPositional));
        if !has_var_positional {
            return false;
        }
    }

    // Check each argument type against param type
    let positional_params: Vec<_> = params
        .iter()
        .filter(|p| {
            matches!(
                p.kind,
                ParamKind::Positional | ParamKind::PositionalOnly | ParamKind::KeywordOnly
            )
        })
        .collect();

    for (i, arg_ty) in arg_types.iter().enumerate() {
        if let Some(param) = positional_params.get(i) {
            if !types_compatible(arg_ty, &param.ty) {
                return false;
            }
        }
    }

    true
}
