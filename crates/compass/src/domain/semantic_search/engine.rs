//! Semantic code search (Sprint 4 - Track 1)
//!
//! Provides type-aware code search capabilities:
//! - Find similar code patterns
//! - Search by type signature
//! - Find implementations of protocol
//! - Call hierarchy analysis

use std::collections::HashMap;
use std::path::PathBuf;

use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::semantic_search::query::{SearchKind, SearchQuery};
use crate::domain::semantic_search::result::{MatchKind, SearchResult};
use crate::type_inference::{DeepTypeInferencer, Type, TypeContext};

mod call_graph;
mod conversion;
mod docstrings;
mod hierarchy_search;
mod indexing;
mod signature_search;
mod symbol_search;

// ============================================================================
// Semantic Search Engine
// ============================================================================

/// Engine for semantic code search.
pub struct SemanticSearchEngine {
    /// Type inferencer
    inferencer: DeepTypeInferencer,
    /// Symbol index
    symbol_index: HashMap<String, Vec<SymbolLocation>>,
    /// Type signature index: signature key -> functions
    type_signature_index: HashMap<String, Vec<SymbolLocation>>,
    /// Call graph
    call_graph: CallGraph,
}

/// Location of a symbol.
#[derive(Debug, Clone)]
pub struct SymbolLocation {
    /// File path
    pub file: PathBuf,
    /// Span in file
    pub span: Span,
    /// Symbol name
    pub name: String,
    /// Symbol kind
    pub kind: MatchKind,
    /// Type (if known)
    pub ty: Option<Type>,
    /// Documentation string (docstring)
    pub docstring: Option<String>,
}

/// Location of a type.
#[derive(Debug, Clone)]
pub struct TypeLocation {
    /// File path
    pub file: PathBuf,
    /// Span in file
    pub span: Span,
    /// Type name
    pub type_name: String,
}

/// A call site in the code.
#[derive(Debug, Clone)]
pub struct CallSite {
    /// File where the call occurs
    pub file: PathBuf,
    /// Location of the call
    pub span: Span,
    /// Function being called
    pub callee: String,
    /// Function containing this call
    pub caller: String,
}

/// Call graph for tracking function calls.
#[derive(Debug, Clone, Default)]
pub struct CallGraph {
    /// Map from function name to calls it makes
    pub calls: HashMap<String, Vec<CallSite>>,
    /// Map from function name to places it's called from
    pub called_by: HashMap<String, Vec<CallSite>>,
}

impl CallGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a call relationship.
    pub fn add_call(&mut self, call_site: CallSite) {
        self.calls
            .entry(call_site.caller.clone())
            .or_default()
            .push(call_site.clone());

        self.called_by
            .entry(call_site.callee.clone())
            .or_default()
            .push(call_site);
    }

    /// Get all functions called by the given function.
    pub fn get_callees(&self, function: &str) -> Vec<&CallSite> {
        self.calls
            .get(function)
            .map(|sites| sites.iter().collect())
            .unwrap_or_default()
    }

    /// Get all functions that call the given function.
    pub fn get_callers(&self, function: &str) -> Vec<&CallSite> {
        self.called_by
            .get(function)
            .map(|sites| sites.iter().collect())
            .unwrap_or_default()
    }
}

impl SemanticSearchEngine {
    /// Create a new search engine.
    pub fn new() -> Self {
        Self {
            inferencer: DeepTypeInferencer::new(),
            symbol_index: HashMap::new(),
            type_signature_index: HashMap::new(),
            call_graph: CallGraph::new(),
        }
    }

    /// Create with type inferencer.
    pub fn with_inferencer(inferencer: DeepTypeInferencer) -> Self {
        Self {
            inferencer,
            symbol_index: HashMap::new(),
            type_signature_index: HashMap::new(),
            call_graph: CallGraph::new(),
        }
    }

    /// Execute a search query.
    pub fn search(&self, query: &SearchQuery) -> SearchResult {
        match &query.kind {
            SearchKind::ByTypeSignature {
                params,
                return_type,
            } => self.search_by_type_signature(params, return_type.as_ref(), query),
            SearchKind::Implementations { protocol } => {
                self.search_implementations(protocol, query)
            }
            SearchKind::Usages { symbol, file } => self.search_usages(symbol, file, query),
            SearchKind::SimilarPatterns { pattern } => self.search_similar_patterns(pattern, query),
            SearchKind::ByDocumentation { query: doc_query } => {
                self.search_by_documentation(doc_query, query)
            }
            SearchKind::CallHierarchy {
                symbol,
                file,
                direction,
            } => self.search_call_hierarchy(symbol, file, *direction, query),
            SearchKind::TypeHierarchy {
                type_name,
                direction,
            } => self.search_type_hierarchy(type_name, *direction, query),
        }
    }

    /// Get type context.
    pub fn type_context(&self) -> &TypeContext {
        self.inferencer.context()
    }
}

impl Default for SemanticSearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
