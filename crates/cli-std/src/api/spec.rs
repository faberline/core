//! `<tool> spec` — the `spec` verb: print a service's API document and run
//! `spec gen`, from parameters.
//!
//! The verb is clap-agnostic: each CLI keeps its own clap registration and
//! calls [`run_print`] or [`run_gen`]. Code generation is a callback
//! ([`run_gen`]'s `generate` argument) because `openapi-codegen` depends on
//! `cli-std`, so `cli-std` cannot depend on it.

pub use crate::application::spec::{
    run_gen, run_print, Format, GenEnd, GenOutcome, GenParams, GenRequest, Lang, NextMarker,
    PrintOptions, Source, SpecParams, EXIT_ERROR, EXIT_OK, EXIT_USAGE,
};
