//! `RefactoringEngine::new`, `with_inferencer` and `Default`: the
//! refactoring engine wired to the tree-sitter parser.

use crate::app::parser::NoParser;
use crate::domain::cross_file::inferencer::DeepTypeInferencer;
use crate::domain::syntax::source_parser::SourceParser;
use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::infrastructure::syntax::multi_parser::MultiParser;

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
