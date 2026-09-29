use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::domain::cross_file::binding::TypeBinding;
use crate::domain::cross_file::file_analysis::FileAnalysis;
use crate::domain::cross_file::import_graph::ImportGraph;
use crate::domain::cross_file::inferencer::trace::TypeTraceStep;
use crate::domain::cross_file::inferencer::DeepTypeInferencer;

impl DeepTypeInferencer {
    /// Run cross-file type propagation for all files in topological order (R1, R2, R3).
    ///
    /// Iterates files in topological order from the internal import graph,
    /// calling `propagate_types()` for each import edge so that downstream
    /// files receive resolved types instead of `Type::Unknown`.
    ///
    /// `cache` maps file paths to their `FileAnalysis` entries.  After this
    /// method returns, each entry's `propagation_complete` flag is set.
    pub fn propagate_all(&mut self, cache: &mut HashMap<PathBuf, FileAnalysis>) {
        // Merge any external cache entries into our internal files map.
        for (path, fa) in cache.iter() {
            if !self.files.contains_key(path) {
                self.files.insert(path.clone(), fa.clone());
            }
        }

        let topo_order = self.import_graph.topological_sort();
        let cycle_members: HashSet<PathBuf> =
            self.detect_import_cycles().into_iter().flatten().collect();

        for file in &topo_order {
            if cycle_members.contains(file) {
                if let Some(fa) = self.files.get_mut(file) {
                    fa.propagation_complete = true;
                }
                continue;
            }

            // Gather import edges for this file from the internal graph.
            let deps: Vec<PathBuf> = self
                .import_graph
                .imports(file)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect();

            for dep in deps {
                // Collect symbol names from the file's imports that reference this dep.
                let sym_names: Vec<String> = self
                    .files
                    .get(file)
                    .map(|fa| {
                        fa.imports
                            .iter()
                            .filter_map(|imp| imp.names.clone())
                            .flatten()
                            .collect()
                    })
                    .unwrap_or_default();

                let syms: Option<Vec<String>> = if sym_names.is_empty() {
                    None
                } else {
                    Some(sym_names)
                };
                let sym_slice: Option<&[String]> = syms.as_deref();

                self.propagate_types(&dep, file, sym_slice);
            }

            if let Some(fa) = self.files.get_mut(file) {
                fa.propagation_complete = true;
            }
        }

        // Write back to the external cache.
        for (path, fa) in &self.files {
            cache.insert(path.clone(), fa.clone());
        }
    }

    /// Detect import cycles in the internal import graph (R9).
    ///
    /// Returns a list of cycles; each cycle is an ordered list of file paths.
    pub fn detect_import_cycles(&self) -> Vec<Vec<PathBuf>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut stack: Vec<PathBuf> = Vec::new();
        let mut on_stack = HashSet::new();

        for file in self.import_graph.all_files() {
            if !visited.contains(&file) {
                Self::dfs_detect_cycles(
                    &self.import_graph,
                    &file,
                    &mut visited,
                    &mut stack,
                    &mut on_stack,
                    &mut cycles,
                );
            }
        }
        cycles
    }

    fn dfs_detect_cycles(
        graph: &ImportGraph,
        node: &PathBuf,
        visited: &mut HashSet<PathBuf>,
        stack: &mut Vec<PathBuf>,
        on_stack: &mut HashSet<PathBuf>,
        cycles: &mut Vec<Vec<PathBuf>>,
    ) {
        visited.insert(node.clone());
        stack.push(node.clone());
        on_stack.insert(node.clone());

        if let Some(deps) = graph.imports(node) {
            for dep in deps.clone() {
                if on_stack.contains(&dep) {
                    // Found a cycle — extract it from the stack.
                    if let Some(pos) = stack.iter().position(|p| p == &dep) {
                        cycles.push(stack[pos..].to_vec());
                    }
                } else if !visited.contains(&dep) {
                    Self::dfs_detect_cycles(graph, &dep, visited, stack, on_stack, cycles);
                }
            }
        }

        stack.pop();
        on_stack.remove(node);
    }

    /// Infer types across all files.
    pub fn infer_all(&mut self) -> Vec<TypeBinding> {
        // Get analysis order
        let order = self.import_graph.topological_sort();

        let mut all_bindings = Vec::new();

        for file in order {
            if let Some(analysis) = self.files.get_mut(&file) {
                // Analyze file
                // This would use the existing TypeInferencer
                analysis.complete = true;

                // Collect bindings
                for binding in analysis.symbols.values() {
                    all_bindings.push(binding.clone());
                }
            }
        }

        all_bindings
    }

    /// Trace a type through function calls.
    pub fn trace_type(&self, symbol: &str, file: &PathBuf) -> Vec<TypeTraceStep> {
        let mut trace = Vec::new();
        let mut visited = HashSet::new();

        self.trace_recursive(symbol, file, &mut trace, &mut visited);

        trace
    }

    fn trace_recursive(
        &self,
        symbol: &str,
        file: &PathBuf,
        trace: &mut Vec<TypeTraceStep>,
        visited: &mut HashSet<(String, PathBuf)>,
    ) {
        let key = (symbol.to_string(), file.clone());
        if visited.contains(&key) {
            return;
        }
        visited.insert(key);

        if let Some(binding) = self.context.get_binding(file, symbol) {
            trace.push(TypeTraceStep {
                symbol: symbol.to_string(),
                file: file.clone(),
                ty: binding.ty.clone(),
                line: binding.line,
            });

            // Follow dependencies
            for dep in &binding.dependencies {
                self.trace_recursive(dep, file, trace, visited);
            }
        }
    }
}
