use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::domain::module_cache::content_hash::ContentHash;
use crate::domain::modules::graph::ModuleGraph;
use crate::domain::modules::import::ModuleInfo;
use crate::infrastructure::module_cache::analysis_cache::AnalysisCache;
use crate::infrastructure::module_cache::entry::CacheEntry;
use crate::infrastructure::project::config::ProjectConfig;
use crate::infrastructure::stubs::loader::StubLoader;
use crate::lens_error::Result;
use crate::syntax::{Language, MultiParser};
use crate::type_inference::{Type, TypeChecker, TypeError};

/// Project analyzer
pub struct ProjectAnalyzer {
    /// Project configuration
    config: ProjectConfig,
    /// Module graph
    graph: ModuleGraph,
    /// Stub loader
    stubs: StubLoader,
    /// Parser for source files
    parser: MultiParser,
    /// Analysis cache
    cache: AnalysisCache,
    /// Analyzed module info
    module_info: HashMap<String, ModuleInfo>,
    /// Type errors by module
    errors: HashMap<String, Vec<TypeError>>,
}

impl ProjectAnalyzer {
    pub fn new(config: ProjectConfig) -> Result<Self> {
        let mut stubs = StubLoader::new();
        stubs.load_builtins();

        let parser = MultiParser::new()?;

        Ok(Self {
            config,
            graph: ModuleGraph::new(),
            stubs,
            parser,
            cache: AnalysisCache::new(),
            module_info: HashMap::new(),
            errors: HashMap::new(),
        })
    }

    /// Create analyzer from project root
    pub fn from_root(root: &Path) -> Result<Self> {
        let config = ProjectConfig::from_pyproject(root);
        Self::new(config)
    }

    /// Discover all Python files in the project
    pub fn discover_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();

        for source_dir in &self.config.source_dirs {
            if source_dir.is_dir() {
                self.discover_in_dir(source_dir, &mut files);
            }
        }

        files
    }

    fn discover_in_dir(&self, dir: &Path, files: &mut Vec<PathBuf>) {
        if self.config.should_exclude(dir) {
            return;
        }

        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                self.discover_in_dir(&path, files);
            } else if path.extension().map(|e| e == "py").unwrap_or(false) {
                if !self.config.should_exclude(&path) {
                    files.push(path);
                }
            }
        }
    }

    /// Build module graph from discovered files
    pub fn build_graph(&mut self) {
        let files = self.discover_files();

        for file in &files {
            if let Some(module_name) = ModuleGraph::path_to_module_name(file, &self.config.root) {
                self.graph.add_module(&module_name, Some(file.clone()));

                // Parse imports from file
                if let Ok(source) = fs::read_to_string(file) {
                    let imports = self.extract_imports(&source);
                    for import in imports {
                        self.graph.add_import(&module_name, &import);
                    }
                }
            }
        }
    }

    /// Extract import statements from Python source
    fn extract_imports(&self, source: &str) -> Vec<String> {
        let mut imports = Vec::new();

        for line in source.lines() {
            let trimmed = line.trim();

            // import module
            if let Some(rest) = trimmed.strip_prefix("import ") {
                for part in rest.split(',') {
                    let module = part.split_whitespace().next().unwrap_or("");
                    let module = module.split(" as ").next().unwrap_or(module);
                    if !module.is_empty() {
                        imports.push(module.to_string());
                    }
                }
            }
            // from module import ...
            else if let Some(rest) = trimmed.strip_prefix("from ") {
                if let Some((module, _)) = rest.split_once(" import ") {
                    let module = module.trim();
                    if !module.is_empty() {
                        imports.push(module.to_string());
                    }
                }
            }
        }

        imports
    }

    /// Analyze all modules in the project
    pub fn analyze(&mut self) -> &HashMap<String, Vec<TypeError>> {
        // Build graph if not already built
        if self.graph.module_names().next().is_none() {
            self.build_graph();
        }

        // Get analysis order
        let order = self.graph.topological_sort().unwrap_or_else(|| {
            // If there are cycles, just analyze in any order
            self.graph.module_names().cloned().collect()
        });

        // Analyze each module
        for module_name in order {
            if let Some(node) = self.graph.get_module(&module_name) {
                if let Some(path) = &node.path {
                    if let Ok(source) = fs::read_to_string(path) {
                        let errors = self.analyze_module(&module_name, &source);
                        if !errors.is_empty() {
                            self.errors.insert(module_name, errors);
                        }
                    }
                }
            }
        }

        &self.errors
    }

    /// Analyze a single module
    fn analyze_module(&mut self, _module_name: &str, source: &str) -> Vec<TypeError> {
        // Parse the source file
        let parsed = match self.parser.parse(source, Language::Python) {
            Some(p) => p,
            None => return vec![], // Skip files that fail to parse
        };

        // Run type checker
        let mut checker = TypeChecker::new(source);
        let diagnostics = checker.check_file(&parsed);

        // Convert diagnostics to TypeErrors
        diagnostics
            .into_iter()
            .filter_map(|d| {
                // Only include type errors
                if d.message.contains("type") || d.message.contains("Type") {
                    Some(TypeError {
                        range: d.range,
                        expected: Type::Unknown,
                        got: Type::Unknown,
                        message: d.message,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get module info by name
    pub fn get_module_info(&self, name: &str) -> Option<&ModuleInfo> {
        self.module_info.get(name)
    }

    /// Get type for a name in a module
    pub fn get_type(&self, module: &str, name: &str) -> Option<&Type> {
        // First check module exports
        if let Some(info) = self.module_info.get(module) {
            if let Some(ty) = info.exports.get(name) {
                return Some(ty);
            }
        }

        // Then check stubs
        if let Some(stub) = self.stubs.get_stub(module) {
            return stub.exports.get(name);
        }

        None
    }

    /// Get all errors
    pub fn all_errors(&self) -> impl Iterator<Item = (&String, &Vec<TypeError>)> {
        self.errors.iter()
    }

    /// Get errors for a specific module
    pub fn module_errors(&self, name: &str) -> Option<&Vec<TypeError>> {
        self.errors.get(name)
    }

    /// Get the module graph
    pub fn graph(&self) -> &ModuleGraph {
        &self.graph
    }

    /// Check for circular imports
    pub fn circular_imports(&self) -> Vec<Vec<String>> {
        self.graph.detect_cycles()
    }

    /// Get the analysis cache
    pub fn cache(&self) -> &AnalysisCache {
        &self.cache
    }

    /// Analyze modules in parallel (for independent modules)
    /// This analyzes modules that have no interdependencies in parallel
    pub fn analyze_parallel(&mut self) -> &HashMap<String, Vec<TypeError>> {
        // Build graph if not already built
        if self.graph.module_names().next().is_none() {
            self.build_graph();
        }

        // Collect modules to analyze with their paths and sources
        let modules_to_analyze: Vec<(String, String)> = self
            .graph
            .modules()
            .filter_map(|(name, node)| {
                // Skip if already cached and not changed
                if let Some(cached) = self.cache.get(name) {
                    if !cached.needs_reanalysis() {
                        // Use cached errors
                        if !cached.errors.is_empty() {
                            return None; // Will handle separately
                        }
                        return None;
                    }
                }

                node.path.as_ref().and_then(|path| {
                    fs::read_to_string(path)
                        .ok()
                        .map(|source| (name.clone(), source))
                })
            })
            .collect();

        // Analyze in parallel using rayon
        let results: Vec<(String, Vec<TypeError>)> = modules_to_analyze
            .par_iter()
            .filter_map(|(name, source)| {
                // Create a new parser for this thread
                let mut parser = match MultiParser::new() {
                    Ok(p) => p,
                    Err(_) => return None,
                };

                let parsed = parser.parse(source, Language::Python)?;
                let mut checker = TypeChecker::new(source);
                let diagnostics = checker.check_file(&parsed);

                let errors: Vec<TypeError> = diagnostics
                    .into_iter()
                    .filter_map(|d| {
                        if d.message.contains("type") || d.message.contains("Type") {
                            Some(TypeError {
                                range: d.range,
                                expected: Type::Unknown,
                                got: Type::Unknown,
                                message: d.message,
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                Some((name.clone(), errors))
            })
            .collect();

        // Merge results
        for (name, errors) in results {
            if !errors.is_empty() {
                self.errors.insert(name, errors);
            }
        }

        &self.errors
    }

    /// Incremental analysis - only analyze changed files and their dependents
    pub fn analyze_incremental(&mut self) -> &HashMap<String, Vec<TypeError>> {
        // Build graph if not already built
        if self.graph.module_names().next().is_none() {
            self.build_graph();
        }

        // Find changed modules
        let changed: Vec<String> = self.cache.get_changed_modules();

        // Get all affected modules (including dependents)
        let mut affected = std::collections::HashSet::new();
        for module in &changed {
            affected.extend(self.cache.get_affected_modules(module));
        }

        // Collect module data first to avoid borrow issues
        let modules_data: Vec<(String, PathBuf, String)> = affected
            .iter()
            .filter_map(|module_name| {
                let node = self.graph.get_module(module_name)?;
                let path = node.path.clone()?;
                let source = fs::read_to_string(&path).ok()?;
                Some((module_name.clone(), path, source))
            })
            .collect();

        // Now analyze each module
        for (module_name, path, source) in modules_data {
            let errors = self.analyze_module(&module_name, &source);

            // Update cache
            let hash = ContentHash::from_content(&source);
            let mut entry = CacheEntry::new(
                module_name.clone(),
                path,
                hash,
                ModuleInfo::new(&module_name),
            );
            entry.errors = errors.clone();
            self.cache.store(entry);

            if !errors.is_empty() {
                self.errors.insert(module_name, errors);
            }
        }

        &self.errors
    }
}

#[cfg(test)]
mod tests;
