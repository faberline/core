//! Markdown lint checker (R4)
//!
//! Provides 10 built-in Markdown lint rules (MD001–MD010) plus:
//! - MD011: **Broken relative link** — relative link target does not exist on
//!   disk (enabled when `MarkdownChecker::with_workspace()` is used).
//! - **Frontmatter validation** — YAML front-matter key: value structure check.
//!
//! Markdown symbols (headings, links, code fences, front-matter fields) come
//! from `compass::semantic::SymbolTableBuilder`, not from this module.

pub use crate::domain::lint::markdown::MarkdownChecker;
