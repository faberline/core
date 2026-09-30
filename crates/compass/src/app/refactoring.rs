//! `RefactoringEngine::new`, `with_inferencer` and `Default`: the
//! refactoring engine wired to the tree-sitter parser.

use crate::domain::cross_file::inferencer::DeepTypeInferencer;
use crate::domain::syntax::language::Language;
use crate::domain::syntax::parsed_file::ParsedFile;
use crate::domain::syntax::source_parser::SourceParser;
use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::infrastructure::syntax::multi_parser::MultiParser;

/// Stands in for the tree-sitter parser when its grammars fail to load:
/// nothing parses, so refactorings that need the AST report
/// "Failed to parse file".
struct NoParser;

impl SourceParser for NoParser {
    fn parse(&mut self, _source: &str, _language: Language) -> Option<ParsedFile> {
        None
    }

    fn line_based(&mut self, source: String, language: Language) -> ParsedFile {
        ParsedFile::line_based(source, language)
    }
}

impl RefactoringEngine {
    /// Create a new refactoring engine.
    pub fn new() -> Self {
        Self::with_inferencer(DeepTypeInferencer::new())
    }

    /// Create with existing type inferencer.
    pub fn with_inferencer(inferencer: DeepTypeInferencer) -> Self {
        let parser: Box<dyn SourceParser + Send + Sync> = match MultiParser::new() {
            Ok(parser) => Box::new(parser),
            Err(e) => {
                eprintln!("Failed to create parser: {}", e);
                Box::new(NoParser)
            }
        };
        Self::with_parser(parser, inferencer)
    }
}

impl Default for RefactoringEngine {
    fn default() -> Self {
        Self::new()
    }
}
