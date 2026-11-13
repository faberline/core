//! Language-neutral OpenAPI intermediate representation shared by every emitter:
//! the document model ([`openapi`]), identifier naming ([`names`]), and the
//! schema-key → type-name map ([`typemap`]).
//!
//! The per-language *operation plan* and *type expressions* live under
//! `crate::emit::<lang>`, since they bake in language-specific type syntax.

pub mod names;
pub mod openapi;
pub mod operations;
// Part 1 of #128 has no caller outside the tests; part 2 wires it into generation.
#[allow(dead_code)]
pub mod select;
pub mod typemap;

pub use typemap::{build_type_map, TypeMap};
