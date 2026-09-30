//! Auto-discovery of project scopes from marker files (#1127)
//!
//! Scans a monorepo root for:
//! - `Cargo.toml` with `[workspace]` → Rust scope
//! - `pyproject.toml` → Python scope (detects .venv for site-packages)
//! - `tsconfig.json` + `package.json` → TypeScript scope

pub use crate::infrastructure::scope_discovery::auto_discover::{discover_scopes, resolve_scope};
