//! The check use case: lint files and directories, optionally followed by
//! cross-file type propagation.
//!
//! Parsing and file access go through the [`SourceParser`] and
//! [`SourceWalker`] ports; `crate::app::check` wires in the tree-sitter
//! parser and the file-system walker behind the public `check_paths`.

use crate::domain::check::file_result::FileResult;
use crate::domain::check::lint_config::LintConfig;
use crate::domain::check::source_walker::SourceWalker;
use crate::domain::import_graph::graph::ImportGraph;
use crate::domain::syntax::language::Language;
use crate::domain::syntax::source_parser::SourceParser;
use crate::lint::CheckerRegistry;
use crate::type_inference::{
    DeepTypeInferencer, PropagationPipeline, PropagationRequest, PropagationResult,
};
use std::path::{Path, PathBuf};

/// Lint each path: a file on its own, a directory by every file below it that
/// the config does not exclude.
pub fn check_sources(
    parser: &mut dyn SourceParser,
    walker: &dyn SourceWalker,
    paths: &[&Path],
    config: &LintConfig,
) -> Vec<FileResult> {
    let registry = CheckerRegistry::new();
    let mut results = Vec::new();

    for path in paths {
        if walker.is_file(path) {
            if let Some(result) = check_file(parser, walker, &registry, path, config) {
                results.push(result);
            }
        } else if walker.is_dir(path) {
            for file in walker.files_under(path) {
                if config.is_excluded(&file) {
                    continue;
                }
                if let Some(result) = check_file(parser, walker, &registry, &file, config) {
                    results.push(result);
                }
            }
        }
    }

    results
}

/// Lint one file, or `None` when its language is unknown or disabled, it
/// cannot be read, or it does not parse.
fn check_file(
    parser: &mut dyn SourceParser,
    walker: &dyn SourceWalker,
    registry: &CheckerRegistry,
    path: &Path,
    config: &LintConfig,
) -> Option<FileResult> {
    let language = parser.detect_language(path)?;

    if !config.is_language_enabled(language) {
        return None;
    }

    let source = walker.read_source(path)?;

    // Some languages (Dockerfile, Markdown, Mermaid) use line-based analysis without tree-sitter.
    // SQL, Proto, GraphQL, TOML now have real AST grammars (R3) so parser.parse() handles them;
    // we fall back to line_based only for the remaining line-only languages.
    let parsed = if let Some(p) = parser.parse(&source, language) {
        p
    } else if matches!(
        language,
        Language::Dockerfile | Language::Markdown | Language::Mdx | Language::Mermaid
    ) {
        // Create a minimal ParsedFile for line-based checkers
        parser.line_based(source, language)
    } else {
        return None;
    };

    let checker = registry.get(language)?;
    let diagnostics = checker.check(&parsed, config);

    Some(FileResult {
        path: path.to_path_buf(),
        language,
        diagnostics,
    })
}

/// Run cross-file type propagation (R10) over the files a check produced.
///
/// Reads each checked file back through `walker`, builds an ImportGraph
/// across them, runs `PropagationPipeline` in topological order, and returns
/// the lint results with the propagation summary.
pub fn propagate_checked(
    walker: &dyn SourceWalker,
    results: Vec<FileResult>,
    project_root: &Path,
) -> (Vec<FileResult>, PropagationResult) {
    // Collect sources for import graph + propagation.
    let mut file_sources: Vec<(PathBuf, String)> = Vec::new();
    for r in &results {
        if let Some(src) = walker.read_source(&r.path) {
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

    // Build ImportGraph.
    let import_graph = ImportGraph::build(&file_sources, project_root);

    // Run propagation pipeline.
    let mut inferencer = DeepTypeInferencer::new();
    let all_files: Vec<PathBuf> = file_sources.iter().map(|(p, _)| p.clone()).collect();

    let request = PropagationRequest {
        files: all_files,
        changed_files: Vec::new(), // full propagation
    };

    let propagation = PropagationPipeline::run(&request, &mut inferencer, &import_graph);

    (results, propagation)
}
