//! The parser port: turns source text into a [`ParsedFile`].
//!
//! The domain and application layers parse through this trait; the
//! tree-sitter implementation with the grammar crates is
//! `MultiParser` in infrastructure/syntax.

use std::path::Path;

use crate::domain::syntax::language::Language;
use crate::domain::syntax::parsed_file::ParsedFile;

/// Parses source text for the languages compass analyses.
pub trait SourceParser {
    /// Parse `source` as `language`.
    ///
    /// Returns `None` when the language has no grammar (Dockerfile,
    /// Markdown, MDX and Mermaid are line-based) or the parse yields no tree.
    fn parse(&mut self, source: &str, language: Language) -> Option<ParsedFile>;

    /// Wrap `source` for the line-based checkers of a language that has no
    /// grammar: the file carries an empty tree and `is_line_based` is true.
    fn line_based(&mut self, source: String, language: Language) -> ParsedFile;

    /// Detect the language of `path`; see [`Language::from_path`].
    fn detect_language(&self, path: &Path) -> Option<Language> {
        Language::from_path(path)
    }
}
