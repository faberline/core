//! Kubernetes structural-schema normalization shared by service CRDs.

pub use crate::infrastructure::crd::{
    add_spec_validation_rule, normalize_unsigned_integer_formats,
    quote_yaml_1_1_boolean_like_strings,
};
