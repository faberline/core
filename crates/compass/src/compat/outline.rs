//! Code outline: enumerate the functions/methods a source file defines.
//!
//! This is a language-agnostic code-intelligence primitive — "what callable
//! definitions live in this file, and where" — built directly on the
//! tree-sitter parse. It deliberately knows nothing about *why* a caller wants
//! the list (instrumentation, navigation, coverage, doc generation); consumers
//! layer their own policy on top. `meter`, for example, maps each
//! [`FunctionDef`] to a probe point.
//!
//! The per-language tree-sitter node-kind knowledge lives here (the compass
//! domain), so consumers never re-implement grammar details.

pub use crate::application::outline::function_outline::{
    outline, outline_parsed, FunctionDef, FunctionKind,
};
