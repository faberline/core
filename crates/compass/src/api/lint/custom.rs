//! Configuration-driven custom linting engine (R3)
//!
//! Loads user-defined lint rules from `cclab/.index/rules.toml` and evaluates
//! them against source files. All custom rule IDs are namespaced with the
//! `CUSTOM_` prefix so they never collide with built-in rule codes.
//!
//! # Rule kinds
//! - `regex` — match a regular expression against the raw source text
//! - `query` — run a tree-sitter named query against the parsed AST
//!
//! # Example `cclab/.index/rules.toml`
//! ```toml
//! [[rule]]
//! id       = "NO_TODO"
//! kind     = "regex"
//! pattern  = "TODO"
//! severity = "warning"
//! message  = "TODO comment found — resolve before merging"
//!
//! [[rule]]
//! id       = "NO_DBG"
//! kind     = "query"
//! pattern  = "(macro_invocation macro: (identifier) @name (#eq? @name \"dbg\"))"
//! severity = "error"
//! message  = "dbg!() macro must not appear in production code"
//! fix      = "Remove the dbg!() call before merging"
//! ```

pub use crate::domain::lint::custom::{
    CustomLintEngine, CustomRuleConfig, CustomRulesFile, RejectedRule, RuleKind,
};
