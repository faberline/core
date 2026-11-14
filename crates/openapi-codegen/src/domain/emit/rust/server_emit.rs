//! Rust server skeleton emitter. #323 adds the handler trait, #324 the error types, #325 the router.

use super::models_emit::HEADER;

/// Contents of the generated `server.rs`: the generated header only for now.
pub fn emit() -> String {
    HEADER.to_string()
}
