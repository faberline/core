//! Emit a Python client: one method per operation, with pydantic-validated
//! sync and async responses. The generated default runtime speaks h2c for
//! http:// and ALPN h2 for https://, while callers may inject any httpx-like
//! object with `request`.

mod client;
mod literal;
mod method;

pub use client::{emit, HEADER};

#[cfg(test)]
mod tests;
