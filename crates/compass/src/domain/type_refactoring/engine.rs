//! Refactoring operations (Sprint 3 - Track 1)
//!
//! Provides type-aware refactoring operations:
//! - Extract function/method/variable
//! - Rename symbol (cross-file)
//! - Move definition
//! - Inline symbol
//! - Change signature

use std::collections::HashMap;
use std::path::PathBuf;

use crate::domain::ast_editing::mutable_ast::MutableAst;
use crate::domain::semantic_search::engine::SemanticSearchEngine;
use crate::domain::type_refactoring::request::{RefactorKind, RefactorRequest};
use crate::domain::type_refactoring::result::RefactorResult;
use crate::type_inference::{DeepTypeInferencer, TypeContext};

mod ast_cache;
mod change_signature;
mod extract_function;
mod extract_method;
mod extract_variable;
mod inline;
mod move_definition;
mod rename;
mod text_analysis;

// ============================================================================
// Refactoring Engine
// ============================================================================

/// Data flow analysis result.
struct DataFlow {
    /// Variables used but not defined in the selection (become parameters)
    external_vars: Vec<String>,
    /// Variables that are defined and used after selection (need to be returned)
    returned_vars: Vec<String>,
}

/// Engine for performing refactoring operations.
pub struct RefactoringEngine {
    /// Type inferencer for type information
    inferencer: DeepTypeInferencer,
    /// AST cache per file
    ast_cache: HashMap<PathBuf, MutableAst>,
    /// Semantic search engine for finding references
    search_engine: SemanticSearchEngine,
}

impl RefactoringEngine {
    /// Create a new refactoring engine.
    pub fn new() -> Self {
        Self {
            inferencer: DeepTypeInferencer::new(),
            ast_cache: HashMap::new(),
            search_engine: SemanticSearchEngine::new(),
        }
    }

    /// Create with existing type inferencer.
    pub fn with_inferencer(inferencer: DeepTypeInferencer) -> Self {
        Self {
            inferencer,
            ast_cache: HashMap::new(),
            search_engine: SemanticSearchEngine::new(),
        }
    }

    // ========================================================================
    // Public Methods
    // ========================================================================

    /// Execute a refactoring operation.
    pub fn execute(&mut self, request: &RefactorRequest, source: &str) -> RefactorResult {
        match &request.kind {
            RefactorKind::ExtractFunction { name } => self.extract_function(request, name, source),
            RefactorKind::ExtractMethod { name } => self.extract_method(request, name, source),
            RefactorKind::ExtractVariable { name } => self.extract_variable(request, name, source),
            RefactorKind::Rename { new_name } => self.rename_symbol(request, new_name, source),
            RefactorKind::MoveDefinition { target_file } => {
                self.move_definition(request, target_file, source)
            }
            RefactorKind::Inline => self.inline_symbol(request, source),
            RefactorKind::ChangeSignature { changes } => {
                self.change_signature(request, changes, source)
            }
        }
    }

    /// Get type context.
    pub fn type_context(&self) -> &TypeContext {
        self.inferencer.context()
    }
}

impl Default for RefactoringEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
