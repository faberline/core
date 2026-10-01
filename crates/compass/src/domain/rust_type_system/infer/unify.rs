use crate::domain::rust_type_system::infer::{
    RustTypeError, RustTypeErrorKind, RustTypeInferencer,
};
use crate::domain::rust_type_system::types::RustType;

impl RustTypeInferencer {
    /// Unify two types
    pub fn unify(&mut self, t1: &RustType, t2: &RustType) -> Result<RustType, RustTypeError> {
        match (t1, t2) {
            // Same types
            (a, b) if a == b => Ok(a.clone()),

            // Infer can unify with anything
            (RustType::Infer, other) | (other, RustType::Infer) => Ok(other.clone()),

            // Type parameters
            (RustType::TypeParam { id, .. }, other) | (other, RustType::TypeParam { id, .. }) => {
                if let Some(existing) = self.substitutions.get(id).cloned() {
                    self.unify(&existing, other)
                } else {
                    self.substitutions.insert(*id, other.clone());
                    Ok(other.clone())
                }
            }

            // References
            (
                RustType::Reference {
                    mutable: m1,
                    inner: i1,
                    ..
                },
                RustType::Reference {
                    mutable: m2,
                    inner: i2,
                    ..
                },
            ) => {
                if m1 != m2 {
                    return Err(RustTypeError {
                        message: "Mutability mismatch".to_string(),
                        span: None,
                        kind: RustTypeErrorKind::TypeMismatch,
                    });
                }
                let unified = self.unify(i1, i2)?;
                Ok(RustType::Reference {
                    lifetime: None,
                    mutable: *m1,
                    inner: Box::new(unified),
                })
            }

            // Tuples
            (RustType::Tuple(t1s), RustType::Tuple(t2s)) => {
                if t1s.len() != t2s.len() {
                    return Err(RustTypeError {
                        message: "Tuple length mismatch".to_string(),
                        span: None,
                        kind: RustTypeErrorKind::TypeMismatch,
                    });
                }
                let unified: Result<Vec<_>, _> = t1s
                    .iter()
                    .zip(t2s.iter())
                    .map(|(a, b)| self.unify(a, b))
                    .collect();
                Ok(RustType::Tuple(unified?))
            }

            // Arrays
            (
                RustType::Array {
                    element: e1,
                    size: s1,
                },
                RustType::Array {
                    element: e2,
                    size: s2,
                },
            ) => {
                if s1 != s2 {
                    return Err(RustTypeError {
                        message: "Array size mismatch".to_string(),
                        span: None,
                        kind: RustTypeErrorKind::TypeMismatch,
                    });
                }
                let unified = self.unify(e1, e2)?;
                Ok(RustType::Array {
                    element: Box::new(unified),
                    size: *s1,
                })
            }

            // Type mismatch
            _ => Err(RustTypeError {
                message: format!("Cannot unify {:?} with {:?}", t1, t2),
                span: None,
                kind: RustTypeErrorKind::TypeMismatch,
            }),
        }
    }

    /// Apply substitutions to a type
    pub fn apply_substitutions(&self, ty: &RustType) -> RustType {
        match ty {
            RustType::TypeParam { id, .. } => {
                if let Some(subst) = self.substitutions.get(id) {
                    self.apply_substitutions(subst)
                } else {
                    ty.clone()
                }
            }
            RustType::Reference {
                lifetime,
                mutable,
                inner,
            } => RustType::Reference {
                lifetime: lifetime.clone(),
                mutable: *mutable,
                inner: Box::new(self.apply_substitutions(inner)),
            },
            RustType::Tuple(elements) => RustType::Tuple(
                elements
                    .iter()
                    .map(|e| self.apply_substitutions(e))
                    .collect(),
            ),
            RustType::Array { element, size } => RustType::Array {
                element: Box::new(self.apply_substitutions(element)),
                size: *size,
            },
            RustType::Named {
                name,
                module,
                type_args,
                lifetime_args,
            } => RustType::Named {
                name: name.clone(),
                module: module.clone(),
                type_args: type_args
                    .iter()
                    .map(|t| self.apply_substitutions(t))
                    .collect(),
                lifetime_args: lifetime_args.clone(),
            },
            _ => ty.clone(),
        }
    }
}
