//! TypeScript-specific type definitions
//!
//! Extends the core type system with TypeScript-specific constructs.

pub use crate::domain::ts_type_system::types::{
    is_assignable_to, MappedTypeModifier, TemplatePart, TsClass, TsConditionalType, TsEnum,
    TsEnumValue, TsInterface, TsMappedType, TsProperty, TsTemplateLiteralType, TsTypeAlias,
    TsTypeContext, TsTypeParam, Visibility,
};
