use crate::domain::type_system::ty::{
    Param, ParamKind, ParamSpecId, Type, TypeVarId, TypeVarTupleId, Variance,
};

impl Type {
    /// Create an Optional type
    pub fn optional(inner: Type) -> Self {
        Type::Optional(Box::new(inner))
    }

    /// Create a List type
    pub fn list(element: Type) -> Self {
        Type::List(Box::new(element))
    }

    /// Create a Dict type
    pub fn dict(key: Type, value: Type) -> Self {
        Type::Dict(Box::new(key), Box::new(value))
    }

    /// Create a Union type, flattening nested unions
    pub fn union(types: Vec<Type>) -> Self {
        let mut flattened = Vec::new();
        for ty in types {
            match ty {
                Type::Union(inner) => flattened.extend(inner),
                Type::Never => {} // Never is identity for union
                other => {
                    if !flattened.contains(&other) {
                        flattened.push(other);
                    }
                }
            }
        }
        match flattened.len() {
            0 => Type::Never,
            // SAFETY: We checked that len() == 1, so pop() will always return Some
            1 => flattened
                .pop()
                .expect("flattened vec has exactly one element"),
            _ => Type::Union(flattened),
        }
    }

    /// Create a simple Callable type
    pub fn callable(params: Vec<Type>, ret: Type) -> Self {
        let params = params
            .into_iter()
            .enumerate()
            .map(|(i, ty)| Param {
                name: format!("_{}", i),
                ty,
                has_default: false,
                kind: ParamKind::Positional,
            })
            .collect();
        Type::Callable {
            params,
            ret: Box::new(ret),
        }
    }

    /// Create a TypeVar (invariant by default)
    pub fn type_var(id: usize, name: &str) -> Type {
        Type::TypeVar {
            id: TypeVarId(id),
            name: name.to_string(),
            bound: None,
            constraints: vec![],
            variance: Variance::Invariant,
        }
    }

    /// Create a TypeVar with a bound (invariant by default)
    pub fn type_var_bounded(id: usize, name: &str, bound: Type) -> Type {
        Type::TypeVar {
            id: TypeVarId(id),
            name: name.to_string(),
            bound: Some(Box::new(bound)),
            constraints: vec![],
            variance: Variance::Invariant,
        }
    }

    /// Create a TypeVar with specific variance
    pub fn type_var_with_variance(id: usize, name: &str, variance: Variance) -> Type {
        Type::TypeVar {
            id: TypeVarId(id),
            name: name.to_string(),
            bound: None,
            constraints: vec![],
            variance,
        }
    }

    /// Create a covariant TypeVar
    pub fn type_var_covariant(id: usize, name: &str) -> Type {
        Type::type_var_with_variance(id, name, Variance::Covariant)
    }

    /// Create a contravariant TypeVar
    pub fn type_var_contravariant(id: usize, name: &str) -> Type {
        Type::type_var_with_variance(id, name, Variance::Contravariant)
    }

    /// Create a ParamSpec (PEP 612)
    pub fn param_spec(id: usize, name: &str) -> Type {
        Type::ParamSpec {
            id: ParamSpecId(id),
            name: name.to_string(),
        }
    }

    /// Create a TypeVarTuple (PEP 646)
    pub fn type_var_tuple(id: usize, name: &str) -> Type {
        Type::TypeVarTuple {
            id: TypeVarTupleId(id),
            name: name.to_string(),
        }
    }

    /// Create a Final type (PEP 591)
    pub fn final_type(inner: Type) -> Type {
        Type::Final(Box::new(inner))
    }

    /// Create an Annotated type (PEP 593)
    pub fn annotated(inner: Type, metadata: Vec<String>) -> Type {
        Type::Annotated {
            inner: Box::new(inner),
            metadata,
        }
    }

    /// Create a Self type (PEP 673)
    pub fn self_type(class_name: Option<String>) -> Type {
        Type::SelfType { class_name }
    }

    /// Create an Overloaded function type
    pub fn overloaded(signatures: Vec<Type>) -> Type {
        Type::Overloaded { signatures }
    }

    /// Create a Concatenate type (PEP 612)
    pub fn concatenate(params: Vec<Type>, param_spec: Type) -> Type {
        Type::Concatenate {
            params,
            param_spec: Box::new(param_spec),
        }
    }

    /// Create a TypeGuard type (PEP 647)
    pub fn type_guard(inner: Type) -> Type {
        Type::TypeGuard(Box::new(inner))
    }

    /// Create a TypeIs type (PEP 742)
    pub fn type_is(inner: Type) -> Type {
        Type::TypeIs(Box::new(inner))
    }
}
