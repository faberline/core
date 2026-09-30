//! `check_paths`: the check use case wired to the tree-sitter parser and the
//! file-system walker.

use std::path::Path;

use crate::application::check::check_paths::{check_sources, propagate_checked};
use crate::domain::check::file_result::FileResult;
use crate::domain::check::lint_config::LintConfig;
use crate::infrastructure::check::source_walker::FsSourceWalker;
use crate::infrastructure::syntax::multi_parser::MultiParser;
use crate::type_inference::PropagationResult;

/// Check files and return diagnostics
pub fn check_paths(paths: &[&Path], config: &LintConfig) -> Vec<FileResult> {
    // Initialize parser, return empty results on failure
    let mut parser = match MultiParser::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to initialize parser: {}", e);
            return Vec::new();
        }
    };

    check_sources(&mut parser, &FsSourceWalker, paths, config)
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
    propagate_checked(&FsSourceWalker, check_paths(paths, config), project_root)
}
