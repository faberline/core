//! Top-level file checking orchestrator
//!
//! Provides the public `check_paths` API and supporting types (`FileResult`,
//! `LintConfig`) that were formerly in `lens/mod.rs`.

use crate::domain::check::file_result::FileResult;
use crate::domain::check::lint_config::LintConfig;
use crate::domain::import_graph::graph::ImportGraph;
use crate::infrastructure::check::source_walker::{check_directory, check_file};
use crate::infrastructure::syntax::multi_parser::MultiParser;
use crate::lint::CheckerRegistry;
use crate::type_inference::{
    DeepTypeInferencer, PropagationPipeline, PropagationRequest, PropagationResult,
};
use std::path::{Path, PathBuf};

/// Check files and return diagnostics
pub fn check_paths(paths: &[&Path], config: &LintConfig) -> Vec<FileResult> {
    let registry = CheckerRegistry::new();

    // Initialize parser, return empty results on failure
    let mut parser = match MultiParser::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to initialize parser: {}", e);
            return Vec::new();
        }
    };

    let mut results = Vec::new();

    for path in paths {
        if path.is_file() {
            if let Some(result) = check_file(&mut parser, &registry, path, config) {
                results.push(result);
            }
        } else if path.is_dir() {
            results.extend(check_directory(&mut parser, &registry, path, config));
        }
    }

    results
}

/// Check files with cross-file type propagation (R10).
///
/// After running per-file checks, builds an ImportGraph across all checked
/// files, runs `PropagationPipeline` in topological order, and returns both
/// the lint results and the propagation summary.
///
/// This is the preferred entry point when cross-file type resolution is
/// desired (e.g., for `type_at` / `hover` accuracy).
pub fn check_paths_with_propagation(
    paths: &[&Path],
    config: &LintConfig,
    project_root: &Path,
) -> (Vec<FileResult>, PropagationResult) {
    // Phase 1: per-file lint + analysis (unchanged).
    let results = check_paths(paths, config);

    // Phase 2: collect sources for import graph + propagation.
    let mut file_sources: Vec<(PathBuf, String)> = Vec::new();
    for r in &results {
        if let Ok(src) = std::fs::read_to_string(&r.path) {
            file_sources.push((r.path.clone(), src));
        }
    }

    if file_sources.is_empty() {
        return (
            results,
            PropagationResult {
                propagated: Default::default(),
                cycles: Vec::new(),
                stats: Default::default(),
            },
        );
    }

    // Phase 3: build ImportGraph.
    let import_graph = ImportGraph::build(&file_sources, project_root);

    // Phase 4: run propagation pipeline.
    let mut inferencer = DeepTypeInferencer::new();
    let all_files: Vec<PathBuf> = file_sources.iter().map(|(p, _)| p.clone()).collect();

    let request = PropagationRequest {
        files: all_files,
        changed_files: Vec::new(), // full propagation
    };

    let propagation = PropagationPipeline::run(&request, &mut inferencer, &import_graph);

    (results, propagation)
}
