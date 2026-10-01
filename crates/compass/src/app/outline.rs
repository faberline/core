//! `outline`: the function outline wired to the tree-sitter parser.

use crate::application::outline::function_outline::{outline_with, FunctionDef};
use crate::domain::error::argus_error::Result;
use crate::domain::syntax::language::Language;
use crate::infrastructure::syntax::multi_parser::MultiParser;

/// Parse `source` as `language` and enumerate its callable definitions.
///
/// Convenience over [`crate::outline_parsed`] that owns the parse. Errors if
/// the parser cannot initialize or the grammar produces no tree.
///
/// This is a language-agnostic code-intelligence primitive — "what callable
/// definitions live in this file, and where" — built directly on the
/// tree-sitter parse. It deliberately knows nothing about *why* a caller wants
/// the list (instrumentation, navigation, coverage, doc generation); consumers
/// layer their own policy on top. `meter`, for example, maps each
/// [`FunctionDef`] to a probe point.
pub fn outline(source: &str, language: Language) -> Result<Vec<FunctionDef>> {
    let mut parser = MultiParser::new()?;
    outline_with(&mut parser, source, language)
}
