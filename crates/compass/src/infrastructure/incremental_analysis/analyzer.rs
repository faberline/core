use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::domain::incremental_analysis::dependency_graph::DependencyGraph;
use crate::domain::module_cache::content_hash::ContentHash;
use crate::infrastructure::incremental_analysis::change_tracker::{ChangeKind, ChangeTracker};
use crate::syntax::{Language, MultiParser};

// ============================================================================
// Incremental Analyzer
// ============================================================================

/// Configuration for incremental analysis.
#[derive(Debug, Clone)]
pub struct IncrementalConfig {
    /// Enable background analysis
    pub background_analysis: bool,
    /// Maximum files to analyze in one batch
    pub batch_size: usize,
    /// Analysis timeout per file
    pub file_timeout: Duration,
    /// Enable persistent cache
    pub persistent_cache: bool,
    /// Cache directory
    pub cache_dir: Option<PathBuf>,
}

impl Default for IncrementalConfig {
    fn default() -> Self {
        Self {
            background_analysis: true,
            batch_size: 100,
            file_timeout: Duration::from_secs(30),
            persistent_cache: true,
            cache_dir: None,
        }
    }
}

/// Result of incremental analysis.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// Files that were analyzed
    pub analyzed_files: Vec<PathBuf>,
    /// Files that were skipped (cache hit)
    pub cached_files: Vec<PathBuf>,
    /// Files that failed
    pub failed_files: Vec<(PathBuf, String)>,
    /// Total analysis time
    pub total_time: Duration,
}

impl AnalysisResult {
    /// Create empty result.
    pub fn empty() -> Self {
        Self {
            analyzed_files: Vec::new(),
            cached_files: Vec::new(),
            failed_files: Vec::new(),
            total_time: Duration::ZERO,
        }
    }

    /// Check if any files failed.
    pub fn has_failures(&self) -> bool {
        !self.failed_files.is_empty()
    }
}

/// Incremental type analyzer.
pub struct IncrementalAnalyzer {
    /// Configuration
    #[allow(dead_code)]
    config: IncrementalConfig,
    /// Change tracker
    change_tracker: ChangeTracker,
    /// Dependency graph
    dep_graph: DependencyGraph,
    /// Analysis cache (file -> result)
    analysis_cache: HashMap<PathBuf, Arc<CachedAnalysis>>,
}

/// Cached analysis result.
#[derive(Debug, Clone)]
pub struct CachedAnalysis {
    /// Content hash when analyzed
    pub hash: ContentHash,
    /// Analysis timestamp
    pub timestamp: Instant,
    /// Dependencies discovered
    pub dependencies: Vec<PathBuf>,
    /// Whether analysis succeeded
    pub success: bool,
}

impl IncrementalAnalyzer {
    /// Create a new incremental analyzer.
    pub fn new(config: IncrementalConfig) -> Self {
        Self {
            config,
            change_tracker: ChangeTracker::new(),
            dep_graph: DependencyGraph::new(),
            analysis_cache: HashMap::new(),
        }
    }

    /// Create with default config.
    pub fn default_config() -> Self {
        Self::new(IncrementalConfig::default())
    }

    /// Record a file change.
    pub fn file_changed(&mut self, file: PathBuf, kind: ChangeKind, hash: ContentHash) {
        self.change_tracker.record_change(file, kind, hash);
    }

    /// Get files that need reanalysis.
    pub fn get_files_to_analyze(&mut self) -> Vec<PathBuf> {
        let changes = self.change_tracker.get_pending_changes();

        let mut to_analyze = HashSet::new();

        for change in changes {
            // Add the changed file
            to_analyze.insert(change.file.clone());

            // Add affected files
            let affected = self.dep_graph.get_affected_files(&change.file);
            to_analyze.extend(affected);
        }

        to_analyze.into_iter().collect()
    }

    /// Run incremental analysis.
    pub fn analyze(&mut self, files: Vec<PathBuf>) -> AnalysisResult {
        let start = Instant::now();
        let mut result = AnalysisResult::empty();

        for file in files {
            // Check cache
            if let Some(cached) = self.analysis_cache.get(&file) {
                if !self.change_tracker.has_changed(&file, &cached.hash) {
                    result.cached_files.push(file);
                    continue;
                }
            }

            // Analyze file
            match self.analyze_file(&file) {
                Ok(analysis) => {
                    // Update dependency graph
                    for dep in &analysis.dependencies {
                        self.dep_graph.add_dependency(file.clone(), dep.clone());
                    }

                    // Cache result
                    self.analysis_cache.insert(file.clone(), Arc::new(analysis));
                    result.analyzed_files.push(file);
                }
                Err(e) => {
                    result.failed_files.push((file, e));
                }
            }
        }

        result.total_time = start.elapsed();
        result
    }

    /// Analyze a single file.
    ///
    /// Performs incremental analysis by:
    /// 1. Reading file content
    /// 2. Computing content hash
    /// 3. Parsing AST
    /// 4. Extracting imports (dependencies)
    /// 5. Caching results
    fn analyze_file(&self, file: &PathBuf) -> Result<CachedAnalysis, String> {
        // 1. Read file content
        let content =
            fs::read_to_string(file).map_err(|e| format!("Failed to read file: {}", e))?;

        // 2. Compute content hash
        let hash = ContentHash::from_content(&content);

        // 3. Detect language
        let language = MultiParser::detect_language(file)
            .ok_or_else(|| format!("Failed to detect language for file: {}", file.display()))?;

        // 4. Parse file to extract imports
        let dependencies = self.extract_dependencies(&content, language)?;

        // 5. Return cached analysis
        Ok(CachedAnalysis {
            hash,
            timestamp: Instant::now(),
            dependencies,
            success: true,
        })
    }

    /// Extract dependencies (imports) from file content.
    fn extract_dependencies(
        &self,
        content: &str,
        language: Language,
    ) -> Result<Vec<PathBuf>, String> {
        let mut parser =
            MultiParser::new().map_err(|e| format!("Failed to create parser: {}", e))?;

        let parsed = parser
            .parse(content, language)
            .ok_or_else(|| "Failed to parse file".to_string())?;

        let mut dependencies = Vec::new();

        // Walk AST to find import statements
        let _cursor = parsed.tree.walk();

        fn visit_node(
            node: &tree_sitter::Node,
            source: &str,
            language: Language,
            dependencies: &mut Vec<PathBuf>,
        ) {
            match language {
                Language::Python => {
                    // Look for import and from..import statements
                    match node.kind() {
                        "import_statement" | "import_from_statement" => {
                            // Extract module name from import
                            if let Some(module_node) = node
                                .child_by_field_name("module_name")
                                .or_else(|| node.child_by_field_name("name"))
                            {
                                let module_name =
                                    &source[module_node.start_byte()..module_node.end_byte()];
                                // Convert module name to file path (simple heuristic)
                                let path =
                                    PathBuf::from(format!("{}.py", module_name.replace(".", "/")));
                                dependencies.push(path);
                            }
                        }
                        _ => {}
                    }
                }
                Language::TypeScript => {
                    // Look for import statements
                    match node.kind() {
                        "import_statement" => {
                            if let Some(source_node) = node.child_by_field_name("source") {
                                let import_path =
                                    &source[source_node.start_byte()..source_node.end_byte()];
                                // Remove quotes
                                let path_str = import_path.trim_matches(|c| c == '"' || c == '\'');
                                let path = PathBuf::from(path_str);
                                dependencies.push(path);
                            }
                        }
                        _ => {}
                    }
                }
                Language::Rust => {
                    // Look for use statements
                    if node.kind() == "use_declaration" {
                        if let Some(path_node) = node.child_by_field_name("argument") {
                            let use_path = &source[path_node.start_byte()..path_node.end_byte()];
                            // Convert Rust module path to file path
                            let path = PathBuf::from(format!("{}.rs", use_path.replace("::", "/")));
                            dependencies.push(path);
                        }
                    }
                }
                Language::JavaScript => {
                    // Same pattern as TypeScript
                    if node.kind() == "import_statement" {
                        if let Some(source_node) = node.child_by_field_name("source") {
                            let import_path =
                                &source[source_node.start_byte()..source_node.end_byte()];
                            let path_str = import_path.trim_matches(|c| c == '"' || c == '\'');
                            dependencies.push(PathBuf::from(path_str));
                        }
                    }
                }
                Language::Go => {
                    // Look for import declarations
                    if node.kind() == "import_spec" {
                        if let Some(path_node) = node.child_by_field_name("path") {
                            let import_path = &source[path_node.start_byte()..path_node.end_byte()];
                            let path_str = import_path.trim_matches('"');
                            dependencies.push(PathBuf::from(path_str));
                        }
                    }
                }
                Language::Html
                | Language::Css
                | Language::Dockerfile
                | Language::Hcl
                | Language::Yaml
                | Language::Markdown
                | Language::Mdx
                | Language::Mermaid
                | Language::Toml
                | Language::Sql
                | Language::Proto
                | Language::GraphQL => {
                    // No import dependency extraction for these languages yet
                }
            }

            // Recursively visit children
            let mut child_cursor = node.walk();
            for child in node.children(&mut child_cursor) {
                visit_node(&child, source, language, dependencies);
            }
        }

        visit_node(
            &parsed.tree.root_node(),
            content,
            language,
            &mut dependencies,
        );

        Ok(dependencies)
    }

    /// Invalidate cache for a file.
    pub fn invalidate(&mut self, file: &PathBuf) {
        self.analysis_cache.remove(file);
    }

    /// Clear all caches.
    pub fn clear_cache(&mut self) {
        self.analysis_cache.clear();
        self.change_tracker.clear();
    }

    /// Get dependency graph.
    pub fn dep_graph(&self) -> &DependencyGraph {
        &self.dep_graph
    }
}

impl Default for IncrementalAnalyzer {
    fn default() -> Self {
        Self::default_config()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests;
