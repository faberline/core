//! Scoped claim-check access tokens (#445).
//!
//! loom's schema layer **signs** a token scoped to one task's keep keys; keep
//! **verifies** it — so a worker can GET/PUT keep directly (bytes never traverse
//! loom) but only within its scope, and only until it expires. HMAC-SHA256 over a
//! base64url(JSON) payload; both sides share a secret (out of band). Kept in one
//! crate so the signer and verifier can never drift.

mod domain;

pub use domain::{sign, verify, Scope};
