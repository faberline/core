mod analysis;
pub(crate) mod trace;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::domain::cross_file::binding::TypeBinding;
use crate::domain::cross_file::context::TypeContext;
use crate::domain::cross_file::file_analysis::{FileAnalysis, ImportInfo};
use crate::domain::cross_file::import_graph::ImportGraph;
use crate::domain::frameworks::registry::FrameworkRegistry;
use crate::domain::package::manager::PackageManagerDetection;
use crate::type_inference::Type;

// ============================================================================
// Deep Type Inferencer
// ============================================================================

/// Deep type inferencer with cross-file support.
pub struct DeepTypeInferencer {
    /// Type context
    context: TypeContext,
    /// Files being analyzed
    files: HashMap<PathBuf, FileAnalysis>,
    /// Import graph
    import_graph: ImportGraph,
    /// Framework type providers
    framework_registry: FrameworkRegistry,
    /// Virtual environment path (from package manager detection)
    pub(crate) venv_path: Option<PathBuf>,
    /// Package manager detection result
    pkg_detection: Option<PackageManagerDetection>,
}

impl DeepTypeInferencer {
    /// Create a new deep type inferencer.
    pub fn new() -> Self {
        Self {
            context: TypeContext::new(),
            files: HashMap::new(),
            import_graph: ImportGraph::new(),
            framework_registry: FrameworkRegistry::new(),
            venv_path: None,
            pkg_detection: None,
        }
    }

    /// Initialize with package manager detection
    ///
    /// This enables:
    /// - Virtual environment-aware import resolution
    /// - Dependency checking for external modules
    pub fn with_package_detection(mut self, detection: PackageManagerDetection) -> Self {
        self.venv_path = detection.venv_path.clone();
        self.pkg_detection = Some(detection);
        self
    }

    /// Get a reference to the framework registry for configuration.
    pub fn framework_registry(&self) -> &FrameworkRegistry {
        &self.framework_registry
    }

    /// Get a mutable reference to the framework registry for configuration.
    pub fn framework_registry_mut(&mut self) -> &mut FrameworkRegistry {
        &mut self.framework_registry
    }

    /// Add a file for analysis.
    pub fn add_file(&mut self, path: PathBuf) {
        self.files.insert(
            path.clone(),
            FileAnalysis {
                path,
                symbols: HashMap::new(),
                imports: Vec::new(),
                complete: false,
                propagation_complete: false,
            },
        );
    }

    /// Get the type context.
    pub fn context(&self) -> &TypeContext {
        &self.context
    }

    /// Get mutable type context.
    pub fn context_mut(&mut self) -> &mut TypeContext {
        &mut self.context
    }

    /// Check if a module is available in dependencies
    ///
    /// Returns true if the module is listed in the package manager dependencies.
    pub fn has_module(&self, module_name: &str) -> bool {
        if let Some(detection) = &self.pkg_detection {
            // Extract package name (first part before dot)
            let package_name = module_name.split('.').next().unwrap_or(module_name);
            detection.has_dependency(package_name)
        } else {
            false
        }
    }

    /// Get package manager detection result
    pub fn package_detection(&self) -> Option<&PackageManagerDetection> {
        self.pkg_detection.as_ref()
    }

    /// Propagate types from imported files to importing files.
    ///
    /// When a symbol is imported from another file, this method resolves the type
    /// from the source file and makes it available in the importing file.
    ///
    /// # Arguments
    /// * `from_file` - The file being imported from
    /// * `to_file` - The file doing the importing
    /// * `symbols` - The symbols being imported (None = import all exported)
    pub fn propagate_types(
        &mut self,
        from_file: &PathBuf,
        to_file: &PathBuf,
        symbols: Option<&[String]>,
    ) {
        // Get symbols from source file
        let source_symbols = match self.files.get(from_file) {
            Some(analysis) => analysis.symbols.clone(),
            None => return, // Source file not analyzed yet
        };

        // Determine which symbols to propagate
        let symbols_to_propagate: Vec<String> = match symbols {
            Some(names) => names.to_vec(),
            None => {
                // Import all exported symbols
                source_symbols
                    .values()
                    .filter(|b| b.is_exported)
                    .map(|b| b.symbol.clone())
                    .collect()
            }
        };

        // Add type bindings to importing file
        for symbol_name in symbols_to_propagate {
            if let Some(binding) = source_symbols.get(&symbol_name) {
                // Create new binding in target file
                let imported_binding = TypeBinding {
                    ty: binding.ty.clone(),
                    source_file: to_file.clone(),
                    symbol: symbol_name.clone(),
                    line: 0,            // Import statement line (could be tracked)
                    is_exported: false, // Imported symbols are not re-exported by default
                    dependencies: binding.dependencies.clone(),
                    is_propagated: true,
                };

                // Add to target file's symbols
                if let Some(target_analysis) = self.files.get_mut(to_file) {
                    target_analysis
                        .symbols
                        .insert(symbol_name.clone(), imported_binding.clone());
                }

                // Add to global type context
                self.context.add_binding(to_file.clone(), imported_binding);
            }
        }

        // Track import relationship
        self.import_graph
            .add_import(to_file.clone(), from_file.clone());
    }

    /// Update a symbol's type and propagate changes to dependent files.
    ///
    /// When a symbol's type changes, this method updates all files that import
    /// this symbol, ensuring type consistency across the codebase.
    ///
    /// # Arguments
    /// * `file` - The file containing the symbol
    /// * `symbol` - The symbol whose type changed
    /// * `new_type` - The new type for the symbol
    pub fn update_symbol_type(&mut self, file: &PathBuf, symbol: &str, new_type: Type) {
        // Update in source file
        if let Some(analysis) = self.files.get_mut(file) {
            if let Some(binding) = analysis.symbols.get_mut(symbol) {
                binding.ty = new_type.clone();
            }
        }

        // Update in type context
        if let Some(binding) = self.context.get_binding(file, symbol) {
            let mut updated_binding = binding.clone();
            updated_binding.ty = new_type.clone();
            self.context.add_binding(file.clone(), updated_binding);
        }

        // Propagate to importing files
        if let Some(importers) = self.import_graph.imported_by(file) {
            for importing_file in importers.clone() {
                // Check if this file imports the changed symbol
                if let Some(analysis) = self.files.get(&importing_file) {
                    if analysis.symbols.contains_key(symbol) {
                        // Update the imported symbol's type
                        self.update_imported_symbol(&importing_file, symbol, new_type.clone());

                        // Recursively propagate to files importing from this file
                        self.update_symbol_type(&importing_file, symbol, new_type.clone());
                    }
                }
            }
        }
    }

    /// Update an imported symbol's type in a file.
    fn update_imported_symbol(&mut self, file: &PathBuf, symbol: &str, new_type: Type) {
        if let Some(analysis) = self.files.get_mut(file) {
            if let Some(binding) = analysis.symbols.get_mut(symbol) {
                binding.ty = new_type.clone();
            }
        }

        // Update in type context
        if let Some(binding) = self.context.get_binding(file, symbol) {
            let mut updated_binding = binding.clone();
            updated_binding.ty = new_type;
            self.context.add_binding(file.clone(), updated_binding);
        }
    }

    /// Add import information to a file.
    ///
    /// This records that a file imports specific symbols from another file,
    /// which is used for cross-file type propagation.
    pub fn add_import(&mut self, file: &PathBuf, import: ImportInfo) {
        if let Some(analysis) = self.files.get_mut(file) {
            analysis.imports.push(import);
        }
    }

    /// Get all symbols from a file (including imported ones).
    pub fn get_file_symbols(&self, file: &PathBuf) -> HashMap<String, Type> {
        match self.files.get(file) {
            Some(analysis) => analysis
                .symbols
                .iter()
                .map(|(name, binding)| (name.clone(), binding.ty.clone()))
                .collect(),
            None => HashMap::new(),
        }
    }

    /// Add a symbol binding to a file's analysis.
    ///
    /// This is useful for testing and for manually populating file symbols.
    pub fn add_file_symbol(&mut self, file: &PathBuf, symbol: String, binding: TypeBinding) {
        if let Some(analysis) = self.files.get_mut(file) {
            analysis.symbols.insert(symbol, binding);
        }
    }

    /// Get file analysis for a specific file (for testing).
    pub fn get_file_analysis(&self, file: &PathBuf) -> Option<&FileAnalysis> {
        self.files.get(file)
    }

    /// Set a symbol's export status in a file.
    pub fn set_symbol_exported(&mut self, file: &PathBuf, symbol: &str, exported: bool) {
        if let Some(analysis) = self.files.get_mut(file) {
            if let Some(binding) = analysis.symbols.get_mut(symbol) {
                binding.is_exported = exported;
            }
        }
    }

    // -- Propagation pipeline helpers (R1-R3, R8, R9) -------------------------

    /// Get immutable reference to a file's analysis (alias for `get_file_analysis`).
    pub fn file_analysis(&self, file: &PathBuf) -> Option<&FileAnalysis> {
        self.files.get(file)
    }

    /// Get mutable reference to a file's analysis.
    pub fn file_analysis_mut(&mut self, file: &PathBuf) -> Option<&mut FileAnalysis> {
        self.files.get_mut(file)
    }

    /// Add an import edge to the internal import graph.
    pub fn add_import_edge(&mut self, from: PathBuf, to: PathBuf) {
        self.import_graph.add_import(from, to);
    }

    /// Get forward dependencies from the internal import graph.
    pub fn import_graph_deps(&self, file: &PathBuf) -> Option<&HashSet<PathBuf>> {
        self.import_graph.imports(file)
    }

    /// Get reverse dependencies from the internal import graph.
    pub fn import_graph_reverse_deps(&self, file: &PathBuf) -> Option<&HashSet<PathBuf>> {
        self.import_graph.imported_by(file)
    }

    /// Return the topological sort order from the internal import graph.
    pub fn topological_sort(&self) -> Vec<PathBuf> {
        self.import_graph.topological_sort()
    }
}

impl Default for DeepTypeInferencer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
