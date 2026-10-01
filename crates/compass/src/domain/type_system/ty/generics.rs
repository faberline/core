use std::collections::HashMap;

use crate::domain::type_system::ty::{Param, Type, TypeVarId};

impl Type {
    /// Substitute type variables with concrete types
    pub fn substitute(&self, substitutions: &std::collections::HashMap<TypeVarId, Type>) -> Type {
        match self {
            Type::TypeVar { id, .. } => substitutions
                .get(id)
                .cloned()
                .unwrap_or_else(|| self.clone()),
            Type::List(elem) => Type::List(Box::new(elem.substitute(substitutions))),
            Type::Dict(k, v) => Type::Dict(
                Box::new(k.substitute(substitutions)),
                Box::new(v.substitute(substitutions)),
            ),
            Type::Set(elem) => Type::Set(Box::new(elem.substitute(substitutions))),
            Type::Tuple(elems) => {
                Type::Tuple(elems.iter().map(|t| t.substitute(substitutions)).collect())
            }
            Type::Optional(inner) => Type::Optional(Box::new(inner.substitute(substitutions))),
            Type::Union(types) => {
                Type::union(types.iter().map(|t| t.substitute(substitutions)).collect())
            }
            Type::Intersection(types) => {
                Type::Intersection(types.iter().map(|t| t.substitute(substitutions)).collect())
            }
            Type::Callable { params, ret } => Type::Callable {
                params: params
                    .iter()
                    .map(|p| Param {
                        name: p.name.clone(),
                        ty: p.ty.substitute(substitutions),
                        has_default: p.has_default,
                        kind: p.kind,
                    })
                    .collect(),
                ret: Box::new(ret.substitute(substitutions)),
            },
            Type::Instance {
                name,
                module,
                type_args,
            } => Type::Instance {
                name: name.clone(),
                module: module.clone(),
                type_args: type_args
                    .iter()
                    .map(|t| t.substitute(substitutions))
                    .collect(),
            },
            // Other types are unchanged
            other => other.clone(),
        }
    }

    /// Collect all type variables in this type
    pub fn type_vars(&self) -> Vec<TypeVarId> {
        let mut vars = Vec::new();
        self.collect_type_vars(&mut vars);
        vars
    }

    fn collect_type_vars(&self, vars: &mut Vec<TypeVarId>) {
        match self {
            Type::TypeVar { id, .. } => {
                if !vars.contains(id) {
                    vars.push(*id);
                }
            }
            Type::List(elem) | Type::Set(elem) | Type::Optional(elem) => {
                elem.collect_type_vars(vars);
            }
            Type::Dict(k, v) => {
                k.collect_type_vars(vars);
                v.collect_type_vars(vars);
            }
            Type::Tuple(elems) | Type::Union(elems) | Type::Intersection(elems) => {
                for elem in elems {
                    elem.collect_type_vars(vars);
                }
            }
            Type::Callable { params, ret } => {
                for param in params {
                    param.ty.collect_type_vars(vars);
                }
                ret.collect_type_vars(vars);
            }
            Type::Instance { type_args, .. } => {
                for arg in type_args {
                    arg.collect_type_vars(vars);
                }
            }
            _ => {}
        }
    }

    /// Unify this type (pattern) with a concrete type, collecting TypeVar bindings
    /// Returns Some(substitutions) on success, None on failure
    pub fn unify(&self, concrete: &Type, subs: &mut HashMap<TypeVarId, Type>) -> bool {
        match (self, concrete) {
            // TypeVar: bind it to the concrete type
            (Type::TypeVar { id, .. }, _) => {
                if let Some(existing) = subs.get(id) {
                    // Already bound - check consistency
                    existing == concrete
                } else {
                    subs.insert(*id, concrete.clone());
                    true
                }
            }

            // Same primitive types
            (Type::Never, Type::Never)
            | (Type::None, Type::None)
            | (Type::Bool, Type::Bool)
            | (Type::Int, Type::Int)
            | (Type::Float, Type::Float)
            | (Type::Str, Type::Str)
            | (Type::Bytes, Type::Bytes)
            | (Type::Any, _)
            | (_, Type::Any)
            | (Type::Unknown, _)
            | (_, Type::Unknown) => true,

            // Numeric widening: int -> float
            (Type::Float, Type::Int) => true,

            // Container types: unify element types
            (Type::List(a), Type::List(b)) => a.unify(b, subs),
            (Type::Set(a), Type::Set(b)) => a.unify(b, subs),
            (Type::Optional(a), Type::Optional(b)) => a.unify(b, subs),
            (Type::Optional(a), b) if !matches!(b, Type::None) => a.unify(b, subs),

            (Type::Dict(k1, v1), Type::Dict(k2, v2)) => k1.unify(k2, subs) && v1.unify(v2, subs),

            (Type::Tuple(a), Type::Tuple(b)) if a.len() == b.len() => {
                a.iter().zip(b.iter()).all(|(t1, t2)| t1.unify(t2, subs))
            }

            // Callable: unify params and return
            (
                Type::Callable {
                    params: p1,
                    ret: r1,
                },
                Type::Callable {
                    params: p2,
                    ret: r2,
                },
            ) if p1.len() == p2.len() => {
                let params_ok = p1
                    .iter()
                    .zip(p2.iter())
                    .all(|(a, b)| a.ty.unify(&b.ty, subs));
                params_ok && r1.unify(r2, subs)
            }

            // Instance types: same name, unify type args
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
            ) if n1 == n2 && a1.len() == a2.len() => {
                a1.iter().zip(a2.iter()).all(|(t1, t2)| t1.unify(t2, subs))
            }

            // Union: concrete must unify with at least one member
            (Type::Union(types), concrete) => types.iter().any(|t| {
                let mut temp_subs = subs.clone();
                if t.unify(concrete, &mut temp_subs) {
                    *subs = temp_subs;
                    true
                } else {
                    false
                }
            }),

            // Same type exactly
            (a, b) if a == b => true,

            _ => false,
        }
    }
}
