//! Index server configuration — scoped toolchain binding (#1127)
//!
//! Supports auto-discovery of project roots from marker files
//! (Cargo.toml, pyproject.toml, tsconfig.json) and per-scope
//! configuration of search paths, interpreters, and cache directories.

pub use crate::domain::index_scope::index_config::{IndexConfig, ScopeConfig, ScopeLang};
