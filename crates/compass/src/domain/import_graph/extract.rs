use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

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

// The patterns scope `\s`, `\w` and `\b` to ASCII with `(?-u:...)`: an import
// keyword is never separated by, say, a no-break space.

fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("import pattern is valid")
}

// -- Python ------------------------------------------------------------------

static PY_FROM: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"^(?-u:\s)*from(?-u:\s)+(\.{0,3}[a-zA-Z0-9_.]*)(?-u:\s)+import(?-u:\b)")
});
static PY_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| compile(r"^(?-u:\s)*import(?-u:\s)+([a-zA-Z0-9_.]+)"));

fn extract_python_imports(source: &str) -> Vec<ExtractedImport> {
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let cap = PY_FROM.captures(line).or_else(|| PY_IMPORT.captures(line));
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

static JS_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    compile(r#"(?:import(?-u:\s)+.*?(?-u:\s)+from(?-u:\s)+|import(?-u:\s)+)['"]([^'"]+)['"]"#)
});
static JS_REQUIRE: LazyLock<Regex> =
    LazyLock::new(|| compile(r#"require\((?-u:\s)*['"]([^'"]+)['"](?-u:\s)*\)"#));
static JS_DYNAMIC_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| compile(r#"import\((?-u:\s)*['"]([^'"]+)['"](?-u:\s)*\)"#));

fn extract_js_imports(source: &str) -> Vec<ExtractedImport> {
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let ln = (i + 1) as u32;
        for re in [&*JS_IMPORT, &*JS_REQUIRE, &*JS_DYNAMIC_IMPORT] {
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

static RS_MOD: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"^(?-u:\s)*(?:pub(?-u:\s)+)?mod(?-u:\s)+([a-zA-Z_][a-zA-Z0-9_]*)(?-u:\s)*;")
});
static RS_USE: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"^(?-u:\s)*(?:pub(?-u:\s)+)?use(?-u:\s)+(crate|super|self)(?:::([a-zA-Z0-9_:]+))?")
});

fn extract_rust_imports(source: &str) -> Vec<ExtractedImport> {
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let ln = (i + 1) as u32;
        if let Some(c) = RS_MOD.captures(line) {
            out.push(ExtractedImport {
                path: format!("mod:{}", &c[1]),
                line: ln,
                language: "rust",
            });
        }
        if let Some(c) = RS_USE.captures(line) {
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

static GO_SINGLE: LazyLock<Regex> =
    LazyLock::new(|| compile(r#"^(?-u:\s)*import(?-u:\s)+"([^"]+)""#));
static GO_BLOCK: LazyLock<Regex> = LazyLock::new(|| compile(r"^(?-u:\s)*import(?-u:\s)*\("));
static GO_BLOCK_LINE: LazyLock<Regex> =
    LazyLock::new(|| compile(r#"^(?-u:\s)*(?:[a-zA-Z_](?-u:\w)*(?-u:\s)+)?"([^"]+)""#));

fn extract_go_imports(source: &str) -> Vec<ExtractedImport> {
    let mut out = Vec::new();
    let mut in_block = false;
    for (i, line) in source.lines().enumerate() {
        let ln = (i + 1) as u32;
        if in_block {
            if line.trim() == ")" {
                in_block = false;
                continue;
            }
            if let Some(c) = GO_BLOCK_LINE.captures(line) {
                out.push(ExtractedImport {
                    path: c[1].to_string(),
                    line: ln,
                    language: "go",
                });
            }
            continue;
        }
        if GO_BLOCK.is_match(line) {
            in_block = true;
            continue;
        }
        if let Some(c) = GO_SINGLE.captures(line) {
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
