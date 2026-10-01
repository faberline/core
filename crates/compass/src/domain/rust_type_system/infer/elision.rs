use crate::domain::rust_type_system::infer::{RustTypeContext, RustTypeInferencer};
use crate::domain::rust_type_system::types::{Lifetime, RustType, SelfParam};

impl RustTypeInferencer {
    /// Apply Rust lifetime elision rules to a function signature.
    ///
    /// Rust's lifetime elision rules (RFC 141):
    /// 1. Each elided lifetime in input position becomes a distinct lifetime parameter.
    /// 2. If there is exactly one input lifetime (elided or explicit), that lifetime
    ///    is assigned to all elided output lifetimes.
    /// 3. If there are multiple input lifetimes but one of them is `&self` or
    ///    `&mut self`, the lifetime of `self` is assigned to all elided output lifetimes.
    ///
    /// Returns the output type with elided lifetimes filled in.
    pub fn apply_lifetime_elision(
        &mut self,
        self_param: &Option<SelfParam>,
        params: &[RustType],
        return_type: &RustType,
    ) -> RustType {
        // Collect all input lifetimes (explicit ones from params)
        let mut input_lifetimes: Vec<Lifetime> = Vec::new();
        let mut self_lifetime: Option<Lifetime> = None;

        // Check self parameter
        match self_param {
            Some(SelfParam::Ref(lt)) => {
                let lt = lt.clone().unwrap_or_else(|| self.context.fresh_lifetime());
                self_lifetime = Some(lt.clone());
                input_lifetimes.push(lt);
            }
            Some(SelfParam::RefMut(lt)) => {
                let lt = lt.clone().unwrap_or_else(|| self.context.fresh_lifetime());
                self_lifetime = Some(lt.clone());
                input_lifetimes.push(lt);
            }
            _ => {}
        }

        // Collect lifetimes from reference parameters
        for param in params {
            Self::collect_input_lifetimes(param, &mut input_lifetimes, &mut self.context);
        }

        // Determine the output lifetime according to elision rules
        let output_lifetime = if let Some(lt) = self_lifetime {
            // Rule 3: self lifetime wins
            Some(lt)
        } else if input_lifetimes.len() == 1 {
            // Rule 2: single input lifetime
            Some(input_lifetimes[0].clone())
        } else {
            // Multiple inputs without self: cannot elide, leave as-is
            None
        };

        // Apply the output lifetime to all elided positions in return type
        if let Some(lt) = output_lifetime {
            Self::fill_elided_lifetimes(return_type, &lt)
        } else {
            return_type.clone()
        }
    }

    /// Collect input lifetimes from a parameter type
    fn collect_input_lifetimes(
        ty: &RustType,
        lifetimes: &mut Vec<Lifetime>,
        ctx: &mut RustTypeContext,
    ) {
        match ty {
            RustType::Reference {
                lifetime, inner, ..
            } => {
                let lt = lifetime.clone().unwrap_or_else(|| ctx.fresh_lifetime());
                lifetimes.push(lt);
                // Recurse into inner type for nested references
                Self::collect_input_lifetimes(inner, lifetimes, ctx);
            }
            RustType::Named {
                lifetime_args,
                type_args,
                ..
            } => {
                lifetimes.extend(lifetime_args.iter().cloned());
                for arg in type_args {
                    Self::collect_input_lifetimes(arg, lifetimes, ctx);
                }
            }
            RustType::Tuple(elements) => {
                for elem in elements {
                    Self::collect_input_lifetimes(elem, lifetimes, ctx);
                }
            }
            RustType::Slice(inner) | RustType::Array { element: inner, .. } => {
                Self::collect_input_lifetimes(inner, lifetimes, ctx);
            }
            _ => {}
        }
    }

    /// Fill in elided lifetimes in an output type with the given lifetime
    fn fill_elided_lifetimes(ty: &RustType, lt: &Lifetime) -> RustType {
        match ty {
            RustType::Reference {
                lifetime,
                mutable,
                inner,
            } => {
                let filled_lt = if lifetime.is_none() {
                    Some(lt.clone())
                } else {
                    lifetime.clone()
                };
                RustType::Reference {
                    lifetime: filled_lt,
                    mutable: *mutable,
                    inner: Box::new(Self::fill_elided_lifetimes(inner, lt)),
                }
            }
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
                    .map(|t| Self::fill_elided_lifetimes(t, lt))
                    .collect(),
                lifetime_args: lifetime_args.clone(),
            },
            RustType::Tuple(elements) => RustType::Tuple(
                elements
                    .iter()
                    .map(|e| Self::fill_elided_lifetimes(e, lt))
                    .collect(),
            ),
            RustType::Slice(inner) => {
                RustType::Slice(Box::new(Self::fill_elided_lifetimes(inner, lt)))
            }
            RustType::Array { element, size } => RustType::Array {
                element: Box::new(Self::fill_elided_lifetimes(element, lt)),
                size: *size,
            },
            other => other.clone(),
        }
    }
}
