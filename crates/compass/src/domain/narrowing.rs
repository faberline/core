//! Type narrowing: conditions, CFG-based narrowing, and TypeVar, protocol and overload resolution.

pub(crate) mod block_env;
pub(crate) mod cfg_pass;
pub(crate) mod condition;
pub(crate) mod condition_parser;
pub(crate) mod match_pattern;
pub(crate) mod narrower;
pub(crate) mod resolution;
