//! The stand-in parser used when the tree-sitter grammars fail to load.

use crate::domain::syntax::language::Language;
use crate::domain::syntax::parsed_file::ParsedFile;
use crate::domain::syntax::source_parser::SourceParser;

/// Stands in for the tree-sitter parser when its grammars fail to load:
/// nothing parses, so refactorings that need the AST report
/// "Failed to parse file" and the language server publishes no
/// diagnostics.
pub(super) struct NoParser;

impl SourceParser for NoParser {
    fn parse(&mut self, _source: &str, _language: Language) -> Option<ParsedFile> {
        None
    }

    fn line_based(&mut self, source: String, language: Language) -> ParsedFile {
        ParsedFile::line_based(source, language)
    }
}
