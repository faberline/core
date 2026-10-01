//! Rust code generators
//!
//! Generators for:
//! - serde (structs with serialization)
//! - sqlx (database models)
//! - axum (route handlers)
//! - reqwest (HTTP client)

pub mod axum;
pub mod reqwest;
pub mod serde;
pub mod sqlx;

pub use self::axum::AxumGenerator;
pub use self::reqwest::ReqwestGenerator;
pub use self::serde::SerdeGenerator;
pub use self::sqlx::SqlxGenerator;

pub use crate::infrastructure::codegen::rust::{format_to_rust_type, type_to_rust};
