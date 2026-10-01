//! Generator registry — dispatches SpecIR to the correct generator.
//!
//! Instead of ad-hoc generator selection, consumers call
//! `registry.generate(spec_ir, ctx)` and the registry finds the first
//! generator whose `can_generate()` returns `true`.
//!
//! SpecIR types now live in `sdd::generate`. This registry accepts
//! `serde_json::Value` to avoid a circular crate dependency.

pub use crate::application::codegen::registry::GeneratorRegistry;
