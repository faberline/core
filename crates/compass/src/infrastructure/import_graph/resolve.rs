use regex_lite::Regex;
use std::path::{Path, PathBuf};

/// Resolve an import path to an absolute file path
pub fn resolve_import(
    import_path: &str,
    from_file: &Path,
    project_root: &Path,
    language: &str,
) -> Option<PathBuf> {
    match language {
        "python" => resolve_python_import(import_path, from_file, project_root),
        "javascript" | "typescript" => resolve_js_import(import_path, from_file, project_root),
        "rust" => resolve_rust_import(import_path, from_file, project_root),
        "go" => resolve_go_import(import_path, from_file, project_root),
        _ => None,
    }
}

fn resolve_python_import(imp: &str, from: &Path, root: &Path) -> Option<PathBuf> {
    let dots = imp.chars().take_while(|c| *c == '.').count();
    if dots > 0 {
        let mut base = from.parent()?.to_path_buf();
        for _ in 1..dots {
            base = base.parent()?.to_path_buf();
        }
        return resolve_py_module(&imp[dots..], &base);
    }
    resolve_py_module(imp, root)
}

fn resolve_py_module(module: &str, base: &Path) -> Option<PathBuf> {
    if module.is_empty() {
        let init = base.join("__init__.py");
        return if init.exists() { Some(init) } else { None };
    }
    let rel: PathBuf = module.split('.').collect();
    for (ext, is_pkg) in [("py", false), ("pyi", false)] {
        let p = base.join(&rel).with_extension(ext);
        if p.exists() && !is_pkg {
            return Some(p);
        }
    }
    let pkg = base.join(&rel).join("__init__.py");
    if pkg.exists() {
        return Some(pkg);
    }
    let stub = base.join(&rel).with_extension("pyi");
    if stub.exists() {
        return Some(stub);
    }
    None
}

fn resolve_js_import(imp: &str, from: &Path, _root: &Path) -> Option<PathBuf> {
    let base = from.parent()?.join(imp);
    if let Some(ext) = base.extension().and_then(|e| e.to_str()) {
        if ["ts", "tsx", "js", "jsx"].contains(&ext) && base.exists() {
            return Some(base);
        }
    }
    for ext in ["ts", "tsx", "js", "jsx"] {
        let c = base.with_extension(ext);
        if c.exists() {
            return Some(c);
        }
    }
    for ext in ["ts", "tsx", "js", "jsx"] {
        let c = base.join("index").with_extension(ext);
        if c.exists() {
            return Some(c);
        }
    }
    None
}

fn resolve_rust_import(imp: &str, from: &Path, root: &Path) -> Option<PathBuf> {
    if let Some(name) = imp.strip_prefix("mod:") {
        let dir = from.parent()?;
        let stem = from.file_stem()?.to_str()?;
        let base = if ["mod", "lib", "main"].contains(&stem) {
            dir.to_path_buf()
        } else {
            dir.join(stem)
        };
        let f = base.join(format!("{name}.rs"));
        if f.exists() {
            return Some(f);
        }
        let m = base.join(name).join("mod.rs");
        if m.exists() {
            return Some(m);
        }
        return None;
    }
    if let Some(rest) = imp.strip_prefix("crate::") {
        let src = find_rust_crate_src(from, root)?;
        return resolve_rs_chain(&rest.split("::").collect::<Vec<_>>(), &src);
    }
    if let Some(rest) = imp.strip_prefix("super::") {
        let dir = from.parent()?;
        let stem = from.file_stem()?.to_str()?;
        let base = if ["mod", "lib", "main"].contains(&stem) {
            dir.parent()?.to_path_buf()
        } else {
            dir.to_path_buf()
        };
        return resolve_rs_chain(&rest.split("::").collect::<Vec<_>>(), &base);
    }
    None
}

fn find_rust_crate_src(file: &Path, root: &Path) -> Option<PathBuf> {
    let mut dir = file.parent()?;
    loop {
        if dir.join("Cargo.toml").exists() {
            let src = dir.join("src");
            return Some(if src.is_dir() { src } else { dir.to_path_buf() });
        }
        if dir == root || dir.parent().is_none() {
            break;
        }
        dir = dir.parent()?;
    }
    None
}

fn resolve_rs_chain(parts: &[&str], base: &Path) -> Option<PathBuf> {
    if parts.is_empty() {
        return None;
    }
    let mut dir = base.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            let f = dir.join(format!("{part}.rs"));
            if f.exists() {
                return Some(f);
            }
            let m = dir.join(part).join("mod.rs");
            if m.exists() {
                return Some(m);
            }
            return None;
        }
        let next = dir.join(part);
        if next.is_dir() {
            dir = next;
        } else {
            return None;
        }
    }
    None
}

fn resolve_go_import(imp: &str, _from: &Path, root: &Path) -> Option<PathBuf> {
    let candidate = root.join(imp);
    if candidate.is_dir() {
        return Some(candidate);
    }
    let go_mod = root.join("go.mod");
    if go_mod.exists() {
        if let Ok(content) = std::fs::read_to_string(&go_mod) {
            let re = Regex::new(r"^\s*module\s+(\S+)").unwrap();
            for line in content.lines() {
                if let Some(c) = re.captures(line) {
                    if let Some(rest) = imp.strip_prefix(&c[1]) {
                        let rest = rest.strip_prefix('/').unwrap_or(rest);
                        let local = root.join(rest);
                        if local.is_dir() {
                            return Some(local);
                        }
                    }
                }
            }
        }
    }
    None
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests;
