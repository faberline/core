//! Emits `runtime.ts` (the fetch or axios base) and `client.ts` (a `createClient`
//! factory with one typed function per operation, taking a grouped `data` arg).

mod client;
mod method;
mod runtime;

pub use client::{emit_client, type_import};
pub use runtime::emit_runtime;

#[cfg(test)]
mod tests;
