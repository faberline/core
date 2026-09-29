use std::collections::HashMap;

use super::PythonChecker;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, Range};
use crate::syntax::ParsedFile;

// ===== Import Sorting Types =====

/// Import classification for isort-like checking
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ImportGroup {
    Future,     // __future__
    Stdlib,     // Standard library
    ThirdParty, // pip packages
    FirstParty, // Local project
}

/// Parsed import info
#[derive(Debug, Clone)]
struct ImportInfo {
    module: String,
    group: ImportGroup,
    line: u32,
    range: Range,
    #[allow(dead_code)]
    is_from_import: bool,
}

impl PythonChecker {
    // ===== Import Sorting Checks (isort-like) =====

    /// Check import sorting and grouping
    pub(super) fn check_import_sorting(&self, file: &ParsedFile) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut imports: Vec<ImportInfo> = Vec::new();
        let mut seen_modules: HashMap<String, Range> = HashMap::new();
        let mut first_non_import_line: Option<u32> = None;

        // Collect all imports
        file.walk(|node, _depth| {
            let is_import = node.kind() == "import_statement";
            let is_from_import = node.kind() == "import_from_statement";

            if is_import || is_from_import {
                let line = node.start_position().row as u32;
                let range = Range::from_node(node);

                // Check if import is after non-import code
                if let Some(first_code) = first_non_import_line {
                    if line > first_code {
                        diagnostics.push(Diagnostic::warning(
                            range.clone(),
                            "PY604",
                            DiagnosticCategory::Style,
                            "Import should be at the top of the file",
                        ));
                    }
                }

                // Extract module name
                let module = self.extract_import_module(node, file);
                if module.is_empty() {
                    return true;
                }

                // Check for duplicate imports
                if let Some(prev_range) = seen_modules.get(&module) {
                    diagnostics.push(Diagnostic::warning(
                        range.clone(),
                        "PY603",
                        DiagnosticCategory::Style,
                        format!("Duplicate import: '{}'", module),
                    ));
                    let _ = prev_range; // suppress unused warning
                } else {
                    seen_modules.insert(module.clone(), range.clone());
                }

                // Classify import group
                let group = Self::classify_import(&module);

                imports.push(ImportInfo {
                    module,
                    group,
                    line,
                    range,
                    is_from_import,
                });
            } else if first_non_import_line.is_none() {
                // Track first non-import, non-comment, non-docstring line
                let kind = node.kind();
                if kind != "comment"
                    && kind != "expression_statement"  // might be docstring
                    && kind != "module"
                    && node.parent().map(|p| p.kind()) == Some("module")
                {
                    first_non_import_line = Some(node.start_position().row as u32);
                }
            }

            true
        });

        // Check sorting within groups
        if imports.len() > 1 {
            let mut prev_group: Option<ImportGroup> = None;
            let mut prev_module: Option<String> = None;
            let mut prev_line: Option<u32> = None;

            for import in &imports {
                // Check if groups are properly separated
                if let Some(pg) = prev_group {
                    if import.group != pg {
                        // Different group - should have blank line between
                        if let Some(pl) = prev_line {
                            if import.line == pl + 1 {
                                diagnostics.push(Diagnostic::new(
                                    import.range.clone(),
                                    crate::diagnostic::DiagnosticSeverity::Hint,
                                    "PY602",
                                    DiagnosticCategory::Style,
                                    format!(
                                        "Add blank line before {} imports",
                                        Self::group_name(import.group)
                                    ),
                                ));
                            }
                        }
                        prev_module = None; // Reset for new group
                    }
                }

                // Check alphabetical order within group
                if let Some(ref pm) = prev_module {
                    if prev_group == Some(import.group) {
                        if import.module.to_lowercase() < pm.to_lowercase() {
                            diagnostics.push(Diagnostic::new(
                                import.range.clone(),
                                crate::diagnostic::DiagnosticSeverity::Hint,
                                "PY601",
                                DiagnosticCategory::Style,
                                format!("Import '{}' should come before '{}'", import.module, pm),
                            ));
                        }
                    }
                }

                prev_group = Some(import.group);
                prev_module = Some(import.module.clone());
                prev_line = Some(import.line);
            }
        }

        diagnostics
    }

    /// Extract module name from import statement
    fn extract_import_module(&self, node: &tree_sitter::Node<'_>, file: &ParsedFile) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "dotted_name" => {
                    return file.node_text(&child).to_string();
                }
                "aliased_import" => {
                    if let Some(name) = child.child_by_field_name("name") {
                        return file.node_text(&name).to_string();
                    }
                }
                _ => {}
            }
        }
        String::new()
    }

    /// Classify an import into a group
    fn classify_import(module: &str) -> ImportGroup {
        let base = module.split('.').next().unwrap_or(module);

        if base == "__future__" {
            return ImportGroup::Future;
        }

        // Python stdlib modules (common ones)
        const STDLIB: &[&str] = &[
            "abc",
            "argparse",
            "ast",
            "asyncio",
            "base64",
            "builtins",
            "collections",
            "concurrent",
            "contextlib",
            "copy",
            "csv",
            "dataclasses",
            "datetime",
            "decimal",
            "enum",
            "functools",
            "glob",
            "gzip",
            "hashlib",
            "heapq",
            "html",
            "http",
            "importlib",
            "inspect",
            "io",
            "itertools",
            "json",
            "logging",
            "math",
            "multiprocessing",
            "operator",
            "os",
            "pathlib",
            "pickle",
            "platform",
            "pprint",
            "queue",
            "random",
            "re",
            "shutil",
            "signal",
            "socket",
            "sqlite3",
            "ssl",
            "string",
            "struct",
            "subprocess",
            "sys",
            "tempfile",
            "textwrap",
            "threading",
            "time",
            "timeit",
            "traceback",
            "types",
            "typing",
            "unittest",
            "urllib",
            "uuid",
            "warnings",
            "weakref",
            "xml",
            "zipfile",
        ];

        if STDLIB.contains(&base) {
            return ImportGroup::Stdlib;
        }

        // Common third-party packages
        const THIRD_PARTY: &[&str] = &[
            "aiohttp",
            "boto3",
            "celery",
            "click",
            "django",
            "fastapi",
            "flask",
            "httpx",
            "jwt",
            "numpy",
            "pandas",
            "pydantic",
            "pytest",
            "redis",
            "requests",
            "rich",
            "scipy",
            "sentry_sdk",
            "sqlalchemy",
            "starlette",
            "tenacity",
            "toml",
            "tqdm",
            "uvicorn",
            "yaml",
        ];

        if THIRD_PARTY.contains(&base) {
            return ImportGroup::ThirdParty;
        }

        // Default: assume first-party (local project)
        ImportGroup::FirstParty
    }

    /// Get display name for import group
    fn group_name(group: ImportGroup) -> &'static str {
        match group {
            ImportGroup::Future => "future",
            ImportGroup::Stdlib => "standard library",
            ImportGroup::ThirdParty => "third-party",
            ImportGroup::FirstParty => "first-party",
        }
    }
}
