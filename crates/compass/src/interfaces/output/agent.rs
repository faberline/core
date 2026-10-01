use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::application::report::{ImportGraphView, SymbolCategory, SymbolTableView, SymbolView};
use crate::domain::check::file_result::FileResult;
use crate::domain::diagnostic::model::{DiagnosticSeverity, Range};

use super::agent_types::{AgentIssue, AgentOutput, AgentStats, SymbolDef};

/// Builder that orchestrates construction of agent-format output.
pub struct AgentOutputBuilder<'a> {
    /// Project root for computing relative paths.
    project_root: &'a Path,
}

impl<'a> AgentOutputBuilder<'a> {
    pub fn new(project_root: &'a Path) -> Self {
        Self { project_root }
    }

    /// Build the complete agent output from analysis results and views.
    ///
    /// `AgentOutputBuilder::build` (in the composition root) builds the
    /// views from symbol tables and the import graph.
    ///
    /// - `results`: lint/check results per file
    /// - `symbol_tables`: per-file symbol table views (keyed by absolute path)
    /// - `imports`: the import paths of the checked files
    pub(crate) fn build_views(
        &self,
        results: &[FileResult],
        symbol_tables: &[(PathBuf, SymbolTableView)],
        imports: &ImportGraphView,
    ) -> AgentOutput {
        let symbols = self.build_symbols(symbol_tables);
        let imports = self.build_imports(results, imports);
        let issues = self.build_issues(results, symbol_tables);
        let impact = self.build_impact(symbol_tables);

        let impact_edges: usize = impact.values().map(|v| v.len()).sum();

        let stats = AgentStats {
            files_checked: results.len(),
            symbols_found: symbols.len(),
            issues_count: issues.len(),
            impact_edges,
        };

        AgentOutput {
            symbols,
            imports,
            issues,
            impact,
            stats,
        }
    }

    /// Build the symbols map from per-file symbol table views.
    ///
    /// Each symbol is keyed by its qualified name (file-relative name for now).
    /// Includes the type signature from the symbol table (R2, R7). The views
    /// hold only user-defined symbols (no imports or parameters).
    fn build_symbols(
        &self,
        symbol_tables: &[(PathBuf, SymbolTableView)],
    ) -> BTreeMap<String, SymbolDef> {
        let mut symbols = BTreeMap::new();

        for (file_path, table) in symbol_tables {
            let rel_path = self.relative_path(file_path);

            for sym in &table.symbols {
                let kind_str = category_to_agent_kind(sym.category);
                let type_sig = sym.type_signature.clone();

                // Use "file_stem.name" as a simple qualified name
                let qualified = format_qualified_name(&rel_path, &sym.name);

                symbols.insert(
                    qualified,
                    SymbolDef {
                        type_sig,
                        file: rel_path.clone(),
                        line: sym.location.start.line + 1, // 0-indexed to 1-indexed
                        kind: kind_str.to_string(),
                    },
                );
            }
        }

        symbols
    }

    /// Build the imports map from the import graph view.
    ///
    /// Maps file path to list of imported symbol qualified names (R3).
    fn build_imports(
        &self,
        results: &[FileResult],
        import_graph: &ImportGraphView,
    ) -> BTreeMap<String, Vec<String>> {
        let mut imports = BTreeMap::new();

        for result in results {
            let deps = import_graph.imports(&result.path);
            if deps.is_empty() {
                continue;
            }

            let rel_path = self.relative_path(&result.path);
            let import_paths: Vec<String> = deps.to_vec();

            if !import_paths.is_empty() {
                imports.insert(rel_path, import_paths);
            }
        }

        imports
    }

    /// Build the issues array from diagnostics with symbol attribution (R4, R6).
    ///
    /// Each diagnostic is attributed to the nearest enclosing symbol via
    /// the symbol ranges. If no enclosing symbol is found,
    /// uses `"<file-level>"`.
    fn build_issues(
        &self,
        results: &[FileResult],
        symbol_tables: &[(PathBuf, SymbolTableView)],
    ) -> Vec<AgentIssue> {
        let mut issues = Vec::new();

        // Build lookup from path to symbol table
        let table_map: BTreeMap<&Path, &SymbolTableView> = symbol_tables
            .iter()
            .map(|(p, t)| (p.as_path(), t))
            .collect();

        for result in results {
            let rel_path = self.relative_path(&result.path);
            let table = table_map.get(result.path.as_path());

            for diag in &result.diagnostics {
                let symbol_name = table
                    .and_then(|t| {
                        find_enclosing_symbol(t, diag.range.start.line, diag.range.start.character)
                    })
                    .unwrap_or_else(|| "<file-level>".to_string());

                issues.push(AgentIssue {
                    severity: severity_to_str(diag.severity).to_string(),
                    symbol: symbol_name,
                    file: rel_path.clone(),
                    line: diag.range.start.line + 1, // 0-indexed to 1-indexed
                    code: diag.code.to_string(),
                    message: diag.message.clone(),
                });
            }
        }

        issues
    }

    /// Build the impact map from symbol table references (R5).
    ///
    /// Groups non-definition references by target symbol, emitting
    /// "file:line" location strings.
    ///
    /// **Limitation**: Currently resolves references within the same file only.
    /// Cross-file references (e.g., file A calls a symbol defined in file B)
    /// require a global symbol index and are not tracked here. The impact map
    /// is accurate for intra-file references; cross-file tracking is deferred.
    fn build_impact(
        &self,
        symbol_tables: &[(PathBuf, SymbolTableView)],
    ) -> BTreeMap<String, Vec<String>> {
        let mut impact: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for (file_path, table) in symbol_tables {
            let rel_path = self.relative_path(file_path);

            // The views hold only references to user-defined symbols
            for reference in &table.references {
                if reference.is_definition {
                    continue;
                }

                let qualified = format_qualified_name(&rel_path, &reference.symbol_name);
                let location = format!("{}:{}", rel_path, reference.location.start.line + 1);

                impact.entry(qualified).or_default().push(location);
            }
        }

        // Deduplicate locations per symbol
        for locations in impact.values_mut() {
            locations.sort();
            locations.dedup();
        }

        impact
    }

    /// Compute relative path from project root.
    fn relative_path(&self, path: &Path) -> String {
        path.strip_prefix(self.project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string()
    }
}

/// Find the nearest enclosing symbol for a position in a symbol table view.
///
/// Iterates the user-defined symbols and finds one whose range contains the
/// position. Returns the symbol name or None if at file level.
fn find_enclosing_symbol(table: &SymbolTableView, line: u32, character: u32) -> Option<String> {
    let mut best: Option<&SymbolView> = None;

    for sym in &table.symbols {
        if sym.location.contains(line, character) {
            // Prefer the most specific (smallest range) enclosing symbol
            if let Some(current_best) = best {
                let current_size = range_size(&current_best.location);
                let new_size = range_size(&sym.location);
                if new_size < current_size {
                    best = Some(sym);
                }
            } else {
                best = Some(sym);
            }
        }
    }

    best.map(|s| s.name.clone())
}

/// Compute approximate range size for comparison.
fn range_size(range: &Range) -> u64 {
    let lines = (range.end.line as u64).saturating_sub(range.start.line as u64);
    let cols = (range.end.character as u64).saturating_sub(range.start.character as u64);
    lines * 1000 + cols
}

/// Map a symbol category to the agent output kind string.
///
/// Uses the schema-defined enum: function, class, method, variable, constant,
/// interface, type_alias, module.
fn category_to_agent_kind(category: SymbolCategory) -> &'static str {
    match category {
        SymbolCategory::Function => "function",
        SymbolCategory::Class => "class",
        SymbolCategory::Interface => "interface",
        SymbolCategory::Variable => "variable",
        SymbolCategory::Constant => "constant",
        SymbolCategory::TypeAlias => "type_alias",
        SymbolCategory::Module => "module",
    }
}

/// Format a simple qualified name from file path and symbol name.
fn format_qualified_name(rel_path: &str, name: &str) -> String {
    // Extract module name from file path (stem without extension)
    let stem = Path::new(rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    format!("{}.{}", stem, name)
}

/// Map DiagnosticSeverity to agent output severity string.
fn severity_to_str(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Information => "info",
        DiagnosticSeverity::Hint => "hint",
    }
}

#[cfg(test)]
mod tests;
