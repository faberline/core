//! Markdown lint checker (R4)
//!
//! Provides 10 built-in Markdown lint rules (MD001–MD010) plus:
//! - MD011: **Broken relative link** — relative link target does not exist on
//!   disk (enabled when `MarkdownChecker::with_workspace()` is used).
//! - **Frontmatter validation** — YAML front-matter key: value structure check.
//! - **`MarkdownSymbolExtractor`** — tree-sitter-md style symbol extraction for
//!   headings, links, code-fence languages, and MDX component references.
//!
//! ## Upgrade path to tree-sitter-md
//!
//! The current implementation uses a high-fidelity line-based structural
//! parser that produces the same symbol / diagnostic output as `tree-sitter-md`
//! would for the rules implemented here.  When `tree-sitter-md` is added as a
//! workspace dependency the `MarkdownSymbolExtractor` can be swapped to use the
//! AST directly without changing any public interfaces.

pub use crate::domain::lint::markdown::symbol::{MarkdownSymbol, MarkdownSymbolExtractor};
pub use crate::domain::lint::markdown::MarkdownChecker;
