//! Language-neutral OpenAPI intermediate representation shared by every emitter:
//! the document model ([`openapi`]), identifier naming ([`names`]), and the
//! schema-key → type-name map ([`typemap`]).
//!
//! The per-language *operation plan* and *type expressions* live under
//! `crate::emit::<lang>`, since they bake in language-specific type syntax.

pub use crate::domain::ir::{build_type_map, TypeMap};

pub mod names {
    pub use crate::domain::ir::names::{
        is_ident, param_access, prop_key, to_camel, to_pascal, to_snake, NameRegistry,
    };
}

pub mod openapi {
    pub use crate::domain::ir::openapi::{
        AdditionalProperties, Components, Info, MediaType, Operation, Parameter, PathItem, RefObj,
        RefOr, RequestBody, Response, Schema, Spec, TypeField,
    };
}

pub mod operations {
    pub use crate::domain::ir::operations::{build, BodyIR, OperationIR, ParamIR};
}

pub mod typemap {
    pub use crate::domain::ir::typemap::{build_type_map, TypeMap};
}
