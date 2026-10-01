use std::path::PathBuf;

use crate::domain::ast_editing::mutable_ast::Span;

// ============================================================================
// Search Results
// ============================================================================

/// Result of a semantic search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Matching items
    pub matches: Vec<SearchMatch>,
    /// Total matches (may be more than returned)
    pub total_count: usize,
    /// Search statistics
    pub stats: SearchStats,
}

/// A single search match.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    /// File containing the match
    pub file: PathBuf,
    /// Span of the match
    pub span: Span,
    /// Symbol name (if applicable)
    pub symbol: Option<String>,
    /// Match kind
    pub kind: MatchKind,
    /// Relevance score (0.0 to 1.0)
    pub score: f64,
    /// Context lines around the match
    pub context: Option<MatchContext>,
}

/// Kind of search match.
#[derive(Debug, Clone)]
pub enum MatchKind {
    /// Function definition
    FunctionDef,
    /// Method definition
    MethodDef,
    /// Class definition
    ClassDef,
    /// Variable assignment
    VariableAssignment,
    /// Import statement
    Import,
    /// Call expression
    Call,
    /// Type annotation
    TypeAnnotation,
    /// Comment/docstring
    Documentation,
}

/// Context around a match.
#[derive(Debug, Clone)]
pub struct MatchContext {
    /// Lines before match
    pub before: Vec<String>,
    /// The matching line(s)
    pub matched: Vec<String>,
    /// Lines after match
    pub after: Vec<String>,
}

/// Search statistics.
#[derive(Debug, Clone, Default)]
pub struct SearchStats {
    /// Files searched
    pub files_searched: usize,
    /// Time taken (milliseconds)
    pub time_ms: u64,
    /// Whether results were truncated
    pub truncated: bool,
}

impl SearchResult {
    /// Create empty result.
    pub fn empty() -> Self {
        Self {
            matches: Vec::new(),
            total_count: 0,
            stats: SearchStats::default(),
        }
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Get number of matches returned.
    pub fn len(&self) -> usize {
        self.matches.len()
    }
}
