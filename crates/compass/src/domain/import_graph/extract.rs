use regex_lite::Regex;
use std::path::Path;

/// An extracted import from source code
#[derive(Debug, Clone)]
pub struct ExtractedImport {
    pub path: String,
    pub line: u32,
    pub language: &'static str,
}

/// Extract import statements from source code based on file extension
pub fn extract_imports(source: &str, file_path: &Path) -> Vec<ExtractedImport> {
    match file_path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "py" | "pyi" => extract_python_imports(source),
        "ts" | "tsx" | "js" | "jsx" => extract_js_imports(source),
        "rs" => extract_rust_imports(source),
        "go" => extract_go_imports(source),
        _ => Vec::new(),
    }
}

// -- Python ------------------------------------------------------------------

fn extract_python_imports(source: &str) -> Vec<ExtractedImport> {
    let re_from = Regex::new(r"^\s*from\s+(\.{0,3}[a-zA-Z0-9_.]*)\s+import\b").unwrap();
    let re_import = Regex::new(r"^\s*import\s+([a-zA-Z0-9_.]+)").unwrap();
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let cap = re_from.captures(line).or_else(|| re_import.captures(line));
        if let Some(c) = cap {
            out.push(ExtractedImport {
                path: c[1].to_string(),
                line: (i + 1) as u32,
                language: "python",
            });
        }
    }
    out
}

// -- JavaScript / TypeScript -------------------------------------------------

fn extract_js_imports(source: &str) -> Vec<ExtractedImport> {
    let re_imp = Regex::new(r#"(?:import\s+.*?\s+from\s+|import\s+)['"]([^'"]+)['"]"#).unwrap();
    let re_req = Regex::new(r#"require\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap();
    let re_dyn = Regex::new(r#"import\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap();
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let ln = (i + 1) as u32;
        for re in [&re_imp, &re_req, &re_dyn] {
            for caps in re.captures_iter(line) {
                let p = caps[1].to_string();
                if p.starts_with('.') || p.starts_with('/') {
                    out.push(ExtractedImport {
                        path: p,
                        line: ln,
                        language: "typescript",
                    });
                }
            }
        }
    }
    out
}

// -- Rust --------------------------------------------------------------------

fn extract_rust_imports(source: &str) -> Vec<ExtractedImport> {
    let re_mod = Regex::new(r"^\s*(?:pub\s+)?mod\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*;").unwrap();
    let re_use =
        Regex::new(r"^\s*(?:pub\s+)?use\s+(crate|super|self)(?:::([a-zA-Z0-9_:]+))?").unwrap();
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let ln = (i + 1) as u32;
        if let Some(c) = re_mod.captures(line) {
            out.push(ExtractedImport {
                path: format!("mod:{}", &c[1]),
                line: ln,
                language: "rust",
            });
        }
        if let Some(c) = re_use.captures(line) {
            let rest = c.get(2).map(|m| m.as_str()).unwrap_or("");
            let full = if rest.is_empty() {
                c[1].to_string()
            } else {
                format!("{}::{rest}", &c[1])
            };
            out.push(ExtractedImport {
                path: full,
                line: ln,
                language: "rust",
            });
        }
    }
    out
}

// -- Go ----------------------------------------------------------------------

fn extract_go_imports(source: &str) -> Vec<ExtractedImport> {
    let re_single = Regex::new(r#"^\s*import\s+"([^"]+)""#).unwrap();
    let re_block = Regex::new(r"^\s*import\s*\(").unwrap();
    let re_line = Regex::new(r#"^\s*(?:[a-zA-Z_]\w*\s+)?"([^"]+)""#).unwrap();
    let mut out = Vec::new();
    let mut in_block = false;
    for (i, line) in source.lines().enumerate() {
        let ln = (i + 1) as u32;
        if in_block {
            if line.trim() == ")" {
                in_block = false;
                continue;
            }
            if let Some(c) = re_line.captures(line) {
                out.push(ExtractedImport {
                    path: c[1].to_string(),
                    line: ln,
                    language: "go",
                });
            }
            continue;
        }
        if re_block.is_match(line) {
            in_block = true;
            continue;
        }
        if let Some(c) = re_single.captures(line) {
            out.push(ExtractedImport {
                path: c[1].to_string(),
                line: ln,
                language: "go",
            });
        }
    }
    out
}

#[cfg(test)]
mod tests;
