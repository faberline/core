//! Cross-file type propagation pipeline.
//!
//! Orchestrates propagation of type bindings across import edges in topological
//! order.  After per-file inference completes, `PropagationPipeline::run`
//! iterates files from leaf dependencies to root entry points, calling
//! `DeepTypeInferencer::propagate_types()` for each import edge so that
//! downstream files receive resolved types instead of `Type::Unknown`.
//!
//! # Requirements
//! - R1: invoke propagate_types() per import edge after per-file inference
//! - R2: topological order — dependencies before dependents
//! - R3: cache propagated types in FileAnalysis.symbols
//! - R7: prefer .pyi stubs over .py sources when present
//! - R8: invalidation + re-propagation on dependency change
//! - R9: detect cycles, mark cycle members, emit diagnostic

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::application::propagation::types::{
    PropagatedType, PropagationRequest, PropagationResult, PropagationStats,
};
use crate::graph::ImportGraph;
use crate::type_inference::{DeepTypeInferencer, ImportInfo};

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

/// Top-level orchestrator for cross-file type propagation.
///
/// Usage:
/// ```ignore
/// let result = PropagationPipeline::run(&files, &mut inferencer, &file_import_graph);
/// ```
pub struct PropagationPipeline;

impl PropagationPipeline {
    /// Run full or incremental propagation.
    ///
    /// 1. Build (or reuse) the DeepTypeInferencer's internal ImportGraph from
    ///    `FileAnalysis.imports`.
    /// 2. Detect cycles → emit diagnostics for cycle members.
    /// 3. Topological-sort the graph.
    /// 4. For each file in order, propagate types from its dependencies.
    /// 5. Mark `FileAnalysis.propagation_complete = true`.
    pub fn run(
        request: &PropagationRequest,
        inferencer: &mut DeepTypeInferencer,
        file_import_graph: &ImportGraph,
    ) -> PropagationResult {
        let start = Instant::now();
        let mut stats = PropagationStats::default();
        let mut propagated: HashMap<PathBuf, Vec<PropagatedType>> = HashMap::new();

        // --- Step 0: ensure all requested files are registered ---------------
        for file in &request.files {
            if inferencer.file_analysis(file).is_none() {
                inferencer.add_file(file.clone());
            }
        }

        // --- Step 1: build internal import graph from FileAnalysis entries ---
        Self::build_internal_graph(inferencer, file_import_graph, &request.files);

        // --- Step 2: detect cycles ------------------------------------------
        let cycles = inferencer.detect_import_cycles();
        stats.cycles_detected = cycles.len();
        let cycle_members: HashSet<PathBuf> =
            cycles.iter().flat_map(|c| c.iter().cloned()).collect();

        // --- Step 3: topological sort ---------------------------------------
        let topo_order = inferencer.topological_sort();

        // --- Step 4: determine work set (full or incremental) ---------------
        let work_set: HashSet<PathBuf> = if request.changed_files.is_empty() {
            // Full propagation — every file is a candidate.
            topo_order.iter().cloned().collect()
        } else {
            // Incremental — changed files plus all their transitive importers.
            let mut affected = HashSet::new();
            for changed in &request.changed_files {
                Self::collect_transitive_importers(inferencer, changed, &mut affected);
                affected.insert(changed.clone());
            }
            affected
        };

        // --- Step 5: propagate in topological order -------------------------
        for file in &topo_order {
            if !work_set.contains(file) {
                continue;
            }
            stats.files_analyzed += 1;

            // Skip cycle members for cross-file propagation (R9).
            if cycle_members.contains(file) {
                // Mark propagation complete even though we couldn't propagate
                // cross-cycle types — locally-inferred types are still valid.
                if let Some(fa) = inferencer.file_analysis_mut(file) {
                    fa.propagation_complete = true;
                }
                continue;
            }

            // Gather import edges for this file.
            let import_sources = Self::resolve_import_sources(inferencer, file);

            for (source_file, symbol_names) in import_sources {
                // R7: prefer .pyi stub if present
                let effective_source = Self::resolve_stub_or_source(&source_file, inferencer);
                let is_stub = effective_source != source_file;
                if is_stub {
                    stats.stubs_used += 1;
                }

                let syms: Option<Vec<String>> = if symbol_names.is_empty() {
                    None
                } else {
                    Some(symbol_names.clone())
                };
                let sym_slice: Option<&[String]> = syms.as_deref();

                inferencer.propagate_types(&effective_source, file, sym_slice);

                // Record propagated types for the result.
                let count_before = propagated.get(file).map_or(0, |v| v.len());
                for name in symbol_names.iter().chain(
                    // If no specific symbols, all exported were propagated.
                    std::iter::empty(),
                ) {
                    if let Some(binding) = inferencer
                        .file_analysis(file)
                        .and_then(|fa| fa.symbols.get(name))
                    {
                        if binding.is_propagated {
                            propagated
                                .entry(file.clone())
                                .or_default()
                                .push(PropagatedType {
                                    symbol: name.clone(),
                                    type_str: format!("{:?}", binding.ty),
                                    source_file: effective_source.clone(),
                                    source_line: binding.line,
                                    is_stub,
                                });
                        }
                    }
                }
                let count_after = propagated.get(file).map_or(0, |v| v.len());
                stats.symbols_propagated += count_after.saturating_sub(count_before);
            }

            // Mark propagation complete (R3).
            if let Some(fa) = inferencer.file_analysis_mut(file) {
                fa.propagation_complete = true;
            }
        }

        // Mark any requested files that weren't in the topo order
        // (e.g., files with no import edges) as propagation complete.
        for file in &request.files {
            if let Some(fa) = inferencer.file_analysis_mut(file) {
                if !fa.propagation_complete {
                    fa.propagation_complete = true;
                    stats.files_analyzed += 1;
                }
            }
        }

        stats.time_ms = start.elapsed().as_millis() as u64;

        PropagationResult {
            propagated,
            cycles,
            stats,
        }
    }

    /// Invalidate propagated types for a changed file and re-propagate (R8).
    ///
    /// 1. Clear propagated symbols from `changed_file` in all its importers.
    /// 2. Re-run `propagate_types` for the changed file → each importer edge.
    /// 3. Cascade via `update_symbol_type` for transitive re-exports.
    pub fn invalidate_and_repropagate(
        changed_file: &Path,
        inferencer: &mut DeepTypeInferencer,
        file_import_graph: &ImportGraph,
    ) -> PropagationResult {
        // Step 1: collect importers (reverse deps).
        let importers: Vec<PathBuf> = file_import_graph
            .dependents(changed_file)
            .into_iter()
            .collect();

        // Step 2: clear propagated bindings from changed_file in each importer.
        for importer in &importers {
            if let Some(fa) = inferencer.file_analysis_mut(importer) {
                let propagated_from_changed: Vec<String> = fa
                    .symbols
                    .iter()
                    .filter(|(_, b)| b.is_propagated && b.source_file == changed_file)
                    .map(|(name, _)| name.clone())
                    .collect();
                for name in &propagated_from_changed {
                    fa.symbols.remove(name);
                }
                fa.propagation_complete = false;
            }
        }

        // Step 3: re-propagate via incremental request.
        let mut all_files: Vec<PathBuf> = importers.clone();
        all_files.push(changed_file.to_path_buf());
        // Also include transitive importers.
        let mut transitive = HashSet::new();
        for importer in &importers {
            Self::collect_transitive_importers(inferencer, importer, &mut transitive);
        }
        all_files.extend(transitive);
        all_files.sort();
        all_files.dedup();

        let request = PropagationRequest {
            files: all_files,
            changed_files: vec![changed_file.to_path_buf()],
        };

        Self::run(&request, inferencer, file_import_graph)
    }

    // -- Helpers --------------------------------------------------------------

    /// Build the DeepTypeInferencer's internal import graph from
    /// `FileAnalysis.imports` entries, correlated with the file-level
    /// `ImportGraph` edges for resolution.
    fn build_internal_graph(
        inferencer: &mut DeepTypeInferencer,
        file_import_graph: &ImportGraph,
        files: &[PathBuf],
    ) {
        for file in files {
            let deps = file_import_graph.dependencies(file);
            for edge in deps {
                if let Some(ref resolved) = edge.resolved {
                    inferencer.add_import_edge(file.clone(), resolved.clone());
                }
            }
        }
    }

    /// Resolve import sources for a file: returns `(source_file, imported_symbol_names)`.
    ///
    /// Correlates each resolved dependency file with the specific symbols
    /// imported from it by matching the `ImportInfo.module` suffix against
    /// the dependency file stem.
    fn resolve_import_sources(
        inferencer: &DeepTypeInferencer,
        file: &PathBuf,
    ) -> Vec<(PathBuf, Vec<String>)> {
        let imports: Vec<ImportInfo> = inferencer
            .file_analysis(file)
            .map(|fa| fa.imports.clone())
            .unwrap_or_default();

        let graph_deps: HashSet<PathBuf> = inferencer
            .import_graph_deps(file)
            .cloned()
            .unwrap_or_default();

        let mut sources = Vec::new();

        for dep in graph_deps {
            // Extract the file stem of the dependency for matching.
            let dep_stem = dep.file_stem().and_then(|s| s.to_str()).unwrap_or("");

            // Correlate: only collect symbol names from imports whose module
            // matches this specific dependency (by checking if the module path
            // ends with the dep file stem, e.g. module="db" matches "db.py").
            let mut sym_names: Vec<String> = Vec::new();
            for imp in &imports {
                let module_tail = imp.module.rsplit('.').next().unwrap_or(&imp.module);
                if module_tail == dep_stem || imp.module == dep_stem {
                    if let Some(names) = &imp.names {
                        sym_names.extend(names.iter().cloned());
                    }
                    // If names is None, it's a module-level import (import X) —
                    // propagate all exported symbols (empty vec signals this).
                }
            }
            sources.push((dep, sym_names));
        }

        sources
    }

    /// Resolve stub: if `{stem}.pyi` exists in the inferencer's file analysis,
    /// return the stub path; otherwise return the original source.
    fn resolve_stub_or_source(source: &PathBuf, inferencer: &DeepTypeInferencer) -> PathBuf {
        if source.extension().map_or(false, |e| e == "py") {
            let stub = source.with_extension("pyi");
            if inferencer.file_analysis(&stub).is_some() {
                return stub;
            }
        }
        source.clone()
    }

    /// Maximum recursion depth for transitive importer collection.
    /// Prevents stack overflow on deep import trees.
    const MAX_TRANSITIVE_DEPTH: usize = 128;

    /// Collect all transitive importers of a file with bounded recursion depth.
    fn collect_transitive_importers(
        inferencer: &DeepTypeInferencer,
        file: &PathBuf,
        out: &mut HashSet<PathBuf>,
    ) {
        Self::collect_transitive_importers_bounded(inferencer, file, out, 0);
    }

    fn collect_transitive_importers_bounded(
        inferencer: &DeepTypeInferencer,
        file: &PathBuf,
        out: &mut HashSet<PathBuf>,
        depth: usize,
    ) {
        if depth >= Self::MAX_TRANSITIVE_DEPTH {
            return;
        }
        if let Some(importers) = inferencer.import_graph_reverse_deps(file) {
            for importer in importers.clone() {
                if out.insert(importer.clone()) {
                    Self::collect_transitive_importers_bounded(
                        inferencer,
                        &importer,
                        out,
                        depth + 1,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
