use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::checker::FileResult;
use crate::diagnostic::DiagnosticSeverity;
use crate::graph::ImportGraph;
use crate::semantic::symbols::{SymbolKind, SymbolTable};

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
        let symbols = self.build_symbols(symbol_tables);
        let imports = self.build_imports(results, import_graph);
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

    /// Build the symbols map from per-file SymbolTables.
    ///
    /// Each symbol is keyed by its qualified name (file-relative name for now).
    /// Includes type signature from SymbolTable `type_info` (R2, R7).
    fn build_symbols(
        &self,
        symbol_tables: &[(PathBuf, SymbolTable)],
    ) -> BTreeMap<String, SymbolDef> {
        let mut symbols = BTreeMap::new();

        for (file_path, table) in symbol_tables {
            let rel_path = self.relative_path(file_path);

            for sym in table.all_symbols() {
                // Skip imports and parameters — only user-defined symbols
                if matches!(sym.kind, SymbolKind::Import | SymbolKind::Parameter) {
                    continue;
                }

                let kind_str = symbol_kind_to_agent_kind(sym.kind);
                let type_sig = sym.type_info.as_ref().map(|t| t.display());

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

    /// Build the imports map from ImportGraph edges.
    ///
    /// Maps file path to list of imported symbol qualified names (R3).
    fn build_imports(
        &self,
        results: &[FileResult],
        import_graph: &ImportGraph,
    ) -> BTreeMap<String, Vec<String>> {
        let mut imports = BTreeMap::new();

        for result in results {
            let deps = import_graph.dependencies(&result.path);
            if deps.is_empty() {
                continue;
            }

            let rel_path = self.relative_path(&result.path);
            let import_paths: Vec<String> =
                deps.iter().map(|edge| edge.import_path.clone()).collect();

            if !import_paths.is_empty() {
                imports.insert(rel_path, import_paths);
            }
        }

        imports
    }

    /// Build the issues array from diagnostics with symbol attribution (R4, R6).
    ///
    /// Each diagnostic is attributed to the nearest enclosing symbol via
    /// binary search on SymbolTable ranges. If no enclosing symbol is found,
    /// uses `"<file-level>"`.
    fn build_issues(
        &self,
        results: &[FileResult],
        symbol_tables: &[(PathBuf, SymbolTable)],
    ) -> Vec<AgentIssue> {
        let mut issues = Vec::new();

        // Build lookup from path to symbol table
        let table_map: BTreeMap<&Path, &SymbolTable> = symbol_tables
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
                    code: diag.code.clone(),
                    message: diag.message.clone(),
                });
            }
        }

        issues
    }

    /// Build the impact map from SymbolTable references (R5).
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
        symbol_tables: &[(PathBuf, SymbolTable)],
    ) -> BTreeMap<String, Vec<String>> {
        let mut impact: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for (file_path, table) in symbol_tables {
            let rel_path = self.relative_path(file_path);

            for reference in table.all_references() {
                if reference.is_definition {
                    continue;
                }

                // Look up the target symbol
                if let Some(sym) = table.get(reference.symbol_id) {
                    // Skip imports and parameters
                    if matches!(sym.kind, SymbolKind::Import | SymbolKind::Parameter) {
                        continue;
                    }

                    let sym_rel_path = self.relative_path(file_path);
                    let qualified = format_qualified_name(&sym_rel_path, &sym.name);
                    let location = format!("{}:{}", rel_path, reference.location.start.line + 1);

                    impact.entry(qualified).or_default().push(location);
                }
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

/// Find the nearest enclosing symbol for a position in a symbol table.
///
/// Iterates all symbols and finds one whose range contains the position.
/// Returns the symbol name or None if at file level.
fn find_enclosing_symbol(table: &SymbolTable, line: u32, character: u32) -> Option<String> {
    let mut best: Option<&crate::semantic::Symbol> = None;

    for sym in table.all_symbols() {
        // Skip imports and parameters
        if matches!(sym.kind, SymbolKind::Import | SymbolKind::Parameter) {
            continue;
        }

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
fn range_size(range: &crate::diagnostic::Range) -> u64 {
    let lines = (range.end.line as u64).saturating_sub(range.start.line as u64);
    let cols = (range.end.character as u64).saturating_sub(range.start.character as u64);
    lines * 1000 + cols
}

/// Map SymbolKind to the agent output kind string.
///
/// Uses the schema-defined enum: function, class, method, variable, constant,
/// interface, type_alias, module.
fn symbol_kind_to_agent_kind(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Class | SymbolKind::Struct | SymbolKind::Enum => "class",
        SymbolKind::Trait | SymbolKind::Interface => "interface",
        SymbolKind::Variable => "variable",
        SymbolKind::Const | SymbolKind::Static => "constant",
        SymbolKind::TypeAlias | SymbolKind::TypeParameter => "type_alias",
        SymbolKind::Module | SymbolKind::Impl => "module",
        // Infrastructure and other kinds default to "variable"
        SymbolKind::Resource
        | SymbolKind::Job
        | SymbolKind::Stage
        | SymbolKind::Port
        | SymbolKind::Label
        | SymbolKind::Selector
        | SymbolKind::Template => "variable",
        // Decorators, macros
        SymbolKind::Decorator | SymbolKind::Macro => "function",
        // Import, Parameter, EnumMember — filtered upstream but handled for exhaustiveness
        SymbolKind::Import | SymbolKind::Parameter | SymbolKind::EnumMember => "variable",
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
