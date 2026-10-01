//! Emit a `reqwest::blocking` Rust client: one method per operation, returning
//! the serde-deserialized response.

mod client;
mod literal;
mod method;

pub use client::{emit, HEADER};

#[cfg(test)]
mod tests;
