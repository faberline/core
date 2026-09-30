//! Go type inference: structs, interfaces, channels, generics, composite
//! literals, and type assertion tracking.

pub use crate::domain::semantic::types::go::{
    ChannelDirection, GenericParam, GoType, GoTypeInference, MethodInfo, TypeAssertion,
};
