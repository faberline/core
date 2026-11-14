//! Emit a `reqwest::blocking` Rust client: one method per operation, returning
//! the serde-deserialized response.

mod client;
mod literal;
mod method;

pub use client::{emit, emit_with, HEADER};
pub(super) use method::{fn_name, is_ndjson, param_ann, response};

#[cfg(test)]
mod tests;
