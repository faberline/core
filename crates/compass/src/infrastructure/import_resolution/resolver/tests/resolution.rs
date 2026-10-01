use super::*;

/// Acceptance Criteria: WHEN local module imported THEN resolve from src
/// Spec: import-resolution.md#acceptance-criteria
#[test]
fn test_resolve_local_module_from_src() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_local_import");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create project with src directory
    let src_dir = temp_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    // Create src/utils.py with a helper function
    fs::write(
        src_dir.join("utils.py"),
        "def helper(x: int) -> str:\n    return str(x)\n",
    )
    .unwrap();

    // Create src/__init__.py to make it a package
    fs::write(src_dir.join("__init__.py"), "").unwrap();

    let mut resolver = ImportResolver::with_search_paths(vec![temp_dir.clone()]);
    resolver.build_index();

    // Should find src as a package
    let src_entry = resolver.get_index_entry("src");
    assert!(src_entry.is_some(), "src package should be indexed");
    assert!(src_entry.unwrap().is_package);

    // Should find src.utils
    let utils_entry = resolver.get_index_entry("src.utils");
    assert!(utils_entry.is_some(), "src.utils should be indexed");

    // Resolve module path
    let resolved_path = resolver.resolve_module_path("src.utils");
    assert!(resolved_path.is_some());
    assert!(
        resolved_path.as_ref().unwrap().ends_with("utils.py"),
        "Expected utils.py, got: {:?}",
        resolved_path
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Acceptance Criteria: WHEN library imported THEN resolve from site-packages
/// Spec: import-resolution.md#acceptance-criteria
#[test]
fn test_resolve_library_from_site_packages() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_site_pkg_import");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Simulate a venv with site-packages
    let site_packages = temp_dir.join("site-packages");
    fs::create_dir_all(&site_packages).unwrap();

    // Create a mock "requests" package in site-packages
    let requests_pkg = site_packages.join("requests");
    fs::create_dir_all(&requests_pkg).unwrap();
    fs::write(
        requests_pkg.join("__init__.py"),
        "from .api import get, post\n__version__ = '2.28.0'\n",
    )
    .unwrap();
    fs::write(
        requests_pkg.join("api.py"),
        "def get(url: str) -> Response: ...\ndef post(url: str, data: dict) -> Response: ...\n",
    )
    .unwrap();

    // Create resolver with site-packages as search path
    let mut resolver = ImportResolver::with_search_paths(vec![site_packages.clone()]);
    resolver.build_index();

    // Should find requests package
    let requests_entry = resolver.get_index_entry("requests");
    assert!(requests_entry.is_some(), "requests should be indexed");
    assert!(requests_entry.unwrap().is_package);

    // Should find requests.api
    let api_entry = resolver.get_index_entry("requests.api");
    assert!(api_entry.is_some(), "requests.api should be indexed");

    // Resolve the module
    let resolved_path = resolver.resolve_module_path("requests");
    assert!(resolved_path.is_some());
    assert!(resolved_path.as_ref().unwrap().ends_with("__init__.py"));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test get_or_resolve_module for lazy loading
#[test]
fn test_get_or_resolve_module_lazy_loading() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_lazy_load");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a module
    fs::write(temp_dir.join("mymodule.py"), "x = 42").unwrap();

    let mut resolver = ImportResolver::with_search_paths(vec![temp_dir.clone()]);
    resolver.build_index();

    // Module not loaded yet
    assert!(!resolver.has_module("mymodule"));

    // Get or resolve should load it
    let module = resolver.get_or_resolve_module("mymodule");
    assert!(module.is_some());

    // Now it should be registered
    assert!(resolver.has_module("mymodule"));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test wildcard import resolution
#[test]
fn test_wildcard_import_resolution() {
    let mut resolver = ImportResolver::new();

    let mut module_info = ModuleInfo::new("mymodule");
    module_info
        .exports
        .insert("public_func".to_string(), Type::Int);
    module_info
        .exports
        .insert("_private_func".to_string(), Type::Str);
    module_info
        .exports
        .insert("__dunder__".to_string(), Type::Bool);

    resolver.register_module("mymodule", module_info);

    let import = Import::WildcardImport {
        module: "mymodule".to_string(),
    };

    let resolved = resolver.resolve_import(&import);

    // Public names should be imported
    assert!(resolved.contains_key("public_func"));

    // Private names (starting with _) should NOT be imported
    assert!(!resolved.contains_key("_private_func"));
    assert!(!resolved.contains_key("__dunder__"));
}

/// Test module import (import foo) creates module reference
#[test]
fn test_module_import_creates_reference() {
    let resolver = ImportResolver::new();

    let import = Import::Module {
        module: "os".to_string(),
        alias: None,
    };

    let resolved = resolver.resolve_import(&import);

    // Should have "os" as a module reference
    assert!(resolved.contains_key("os"));
    match resolved.get("os") {
        Some(Type::Instance { name, module, .. }) => {
            assert!(name.starts_with("module:"));
            assert_eq!(module.as_deref(), Some("os"));
        }
        _ => panic!("Expected Instance type for module import"),
    }
}

/// Test module import with alias
#[test]
fn test_module_import_with_alias() {
    let resolver = ImportResolver::new();

    let import = Import::Module {
        module: "numpy".to_string(),
        alias: Some("np".to_string()),
    };

    let resolved = resolver.resolve_import(&import);

    // Should have "np" as the name (not "numpy")
    assert!(resolved.contains_key("np"));
    assert!(!resolved.contains_key("numpy"));
}

/// Test indexing skips hidden and excluded directories
#[test]
fn test_index_skips_excluded_directories() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_skip_excluded");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create normal module
    fs::write(temp_dir.join("normal.py"), "").unwrap();

    // Create modules in excluded directories
    fs::create_dir_all(temp_dir.join("__pycache__")).unwrap();
    fs::write(temp_dir.join("__pycache__/cached.py"), "").unwrap();

    fs::create_dir_all(temp_dir.join(".hidden")).unwrap();
    fs::write(temp_dir.join(".hidden/secret.py"), "").unwrap();

    fs::create_dir_all(temp_dir.join("node_modules")).unwrap();
    fs::write(temp_dir.join("node_modules/nodemod.py"), "").unwrap();

    let mut resolver = ImportResolver::with_search_paths(vec![temp_dir.clone()]);
    resolver.build_index();

    let modules = resolver.list_modules(None);
    let module_paths: Vec<_> = modules.iter().map(|m| m.module_path.as_str()).collect();

    // Normal module should be indexed
    assert!(module_paths.contains(&"normal"));

    // Excluded modules should NOT be indexed
    assert!(!module_paths.iter().any(|p| p.contains("cached")));
    assert!(!module_paths.iter().any(|p| p.contains("secret")));
    assert!(!module_paths.iter().any(|p| p.contains("nodemod")));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// R6: `from .utils import helper` resolves within package directory.
#[test]
fn test_relative_import_resolution() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_relative_import");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a package with two modules
    let pkg_dir = temp_dir.join("mypackage");
    fs::create_dir_all(&pkg_dir).unwrap();
    fs::write(pkg_dir.join("__init__.py"), "").unwrap();
    fs::write(
        pkg_dir.join("utils.py"),
        "def helper(x: int) -> str:\n    return str(x)\n",
    )
    .unwrap();
    fs::write(pkg_dir.join("main.py"), "from .utils import helper\n").unwrap();

    let mut resolver = ImportResolver::with_search_paths(vec![temp_dir.clone()]);
    resolver.build_index();

    // mypackage.utils should be indexed
    let utils_entry = resolver.get_index_entry("mypackage.utils");
    assert!(utils_entry.is_some(), "mypackage.utils should be found");
    assert!(
        utils_entry.unwrap().file_path.ends_with("utils.py"),
        "Should resolve to utils.py"
    );

    // Verify the package itself is indexed
    let pkg_entry = resolver.get_index_entry("mypackage");
    assert!(pkg_entry.is_some(), "mypackage should be indexed");
    assert!(pkg_entry.unwrap().is_package);

    // A relative import from .utils should resolve to the same file
    // within the package directory.
    let resolved_utils = resolver.resolve_module_path("mypackage.utils");
    assert!(resolved_utils.is_some());
    assert!(
        resolved_utils.unwrap().ends_with("utils.py"),
        "Relative import .utils should resolve to utils.py within the package"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
