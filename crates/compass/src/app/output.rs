//! `AgentOutputBuilder::build` and `Reporter::generate_agent`: the agent
//! output from symbol tables and the import graph, which are turned into
//! the application report views first.

use std::path::{Path, PathBuf};

use crate::application::report::{ImportGraphView, SymbolTableView};
use crate::domain::check::file_result::FileResult;
use crate::domain::import_graph::graph::ImportGraph;
use crate::domain::semantic::symbols::SymbolTable;
use crate::interfaces::output::agent::AgentOutputBuilder;
use crate::interfaces::output::agent_types::AgentOutput;
use crate::interfaces::output::reporter::Reporter;

/// The report views of the symbol tables and of the checked files' imports.
fn views(
    results: &[FileResult],
    symbol_tables: &[(PathBuf, SymbolTable)],
    import_graph: &ImportGraph,
) -> (Vec<(PathBuf, SymbolTableView)>, ImportGraphView) {
    let tables = symbol_tables
        .iter()
        .map(|(path, table)| (path.clone(), SymbolTableView::of(table)))
        .collect();
    let imports = ImportGraphView::of(import_graph, results.iter().map(|r| r.path.as_path()));
    (tables, imports)
}

impl<'a> AgentOutputBuilder<'a> {
    /// Build the complete agent output from analysis results.
    ///
    /// - `results`: lint/check results per file
    /// - `symbol_tables`: per-file symbol tables (keyed by absolute path)
    /// - `import_graph`: project-wide import dependency graph
    pub fn build(
        &self,
        results: &[FileResult],
        symbol_tables: &[(PathBuf, SymbolTable)],
        import_graph: &ImportGraph,
    ) -> AgentOutput {
        let (tables, imports) = views(results, symbol_tables, import_graph);
        self.build_views(results, &tables, &imports)
    }
}

impl Reporter {
    /// Generate agent-format output with additional SymbolTable and ImportGraph data.
    ///
    /// Agent format requires per-file symbol tables and a project-wide import graph
    /// in addition to the lint results. This method delegates to `AgentOutputBuilder`
    /// to produce symbol-centric JSON optimized for LLM agent consumption.
    ///
    /// # Arguments
    /// - `results` — lint/check results per file
    /// - `symbol_tables` — per-file symbol tables (keyed by absolute path)
    /// - `import_graph` — project-wide import dependency graph
    /// - `project_root` — project root for computing relative paths
    pub fn generate_agent(
        &self,
        results: &[FileResult],
        symbol_tables: &[(PathBuf, SymbolTable)],
        import_graph: &ImportGraph,
        project_root: &Path,
    ) -> String {
        let (tables, imports) = views(results, symbol_tables, import_graph);
        self.generate_agent_views(results, &tables, &imports, project_root)
    }
}
