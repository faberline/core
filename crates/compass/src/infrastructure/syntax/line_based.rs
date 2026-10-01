use crate::domain::syntax::language::Language;
use crate::domain::syntax::parsed_file::ParsedFile;
use tree_sitter::Parser;

impl ParsedFile {
    /// Create a ParsedFile for line-based analysis (no tree-sitter)
    /// Used for languages like Dockerfile that don't have compatible grammars
    pub fn line_based(source: String, language: Language) -> Self {
        // Parse with a dummy parser that produces a minimal tree
        let mut parser = Parser::new();
        // Use a simple grammar to get a valid tree structure
        let _ = parser.set_language(&tree_sitter_html::LANGUAGE.into());
        let tree = parser.parse("", None).expect("empty parse should succeed");
        Self {
            has_errors: false,
            source,
            tree,
            language,
            is_line_based: true,
        }
    }
}
