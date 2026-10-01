//! Incremental analysis (Sprint 5 - Track 1)
//!
//! Provides incremental type checking and analysis:
//! - Dependency tracking
//! - Minimal reanalysis on changes
//! - Persistent analysis cache
//! - Background analysis

pub use crate::domain::incremental_analysis::dependency_graph::DependencyGraph;
pub use crate::infrastructure::incremental_analysis::analyzer::{
    AnalysisResult, CachedAnalysis, IncrementalAnalyzer, IncrementalConfig,
};
pub use crate::infrastructure::incremental_analysis::change_tracker::{
    ChangeKind, ChangeTracker, FileChange,
};
