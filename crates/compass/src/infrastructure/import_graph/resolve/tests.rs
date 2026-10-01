use super::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_resolve_relative_python_import() {
    let tmp = TempDir::new().unwrap();
    let pkg = tmp.path().join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("a.py"), "from .utils import helper").unwrap();
    fs::write(pkg.join("utils.py"), "def helper(): pass").unwrap();
    assert_eq!(
        resolve_python_import(".utils", &pkg.join("a.py"), tmp.path()),
        Some(pkg.join("utils.py")),
    );
}

#[test]
fn test_resolve_relative_js_import() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("app.ts"), "").unwrap();
    fs::write(src.join("utils.ts"), "").unwrap();
    assert_eq!(
        resolve_js_import("./utils", &src.join("app.ts"), tmp.path()),
        Some(src.join("utils.ts")),
    );
}
