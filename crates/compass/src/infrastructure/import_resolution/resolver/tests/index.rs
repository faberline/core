use super::*;

#[test]
fn test_build_index_with_modules() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_import_index");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create module files
    fs::write(temp_dir.join("utils.py"), "def helper(): pass").unwrap();
    fs::write(temp_dir.join("config.py"), "DEBUG = True").unwrap();

    // Create a package
    fs::create_dir_all(temp_dir.join("mypackage")).unwrap();
    fs::write(temp_dir.join("mypackage/__init__.py"), "").unwrap();
    fs::write(temp_dir.join("mypackage/submodule.py"), "class Foo: pass").unwrap();

    let mut resolver = ImportResolver::with_search_paths(vec![temp_dir.clone()]);
    resolver.build_index();

    assert!(resolver.is_indexed());

    // Check indexed modules
    let modules = resolver.list_modules(None);
    assert!(modules.iter().any(|m| m.module_path == "utils"));
    assert!(modules.iter().any(|m| m.module_path == "config"));
    assert!(modules.iter().any(|m| m.module_path == "mypackage"));
    assert!(modules
        .iter()
        .any(|m| m.module_path == "mypackage.submodule"));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_stub_file_priority() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_stub_priority");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create both .py and .pyi
    fs::write(temp_dir.join("mymod.py"), "def foo(): pass").unwrap();
    fs::write(temp_dir.join("mymod.pyi"), "def foo() -> int: ...").unwrap();

    let mut resolver = ImportResolver::with_search_paths(vec![temp_dir.clone()]);
    resolver.build_index();

    // .pyi should be preferred
    let entry = resolver.get_index_entry("mymod");
    assert!(entry.is_some());
    assert!(entry.unwrap().is_stub);

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_list_modules_with_prefix() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_list_prefix");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create Django-like structure
    fs::create_dir_all(temp_dir.join("django")).unwrap();
    fs::write(temp_dir.join("django/__init__.py"), "").unwrap();
    fs::create_dir_all(temp_dir.join("django/db")).unwrap();
    fs::write(temp_dir.join("django/db/__init__.py"), "").unwrap();
    fs::write(temp_dir.join("django/db/models.py"), "").unwrap();

    let mut resolver = ImportResolver::with_search_paths(vec![temp_dir.clone()]);
    resolver.build_index();

    // List all django modules
    let django_modules = resolver.list_modules(Some("django"));
    assert!(django_modules.iter().any(|m| m.module_path == "django"));
    assert!(django_modules.iter().any(|m| m.module_path == "django.db"));
    assert!(django_modules
        .iter()
        .any(|m| m.module_path == "django.db.models"));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_resolve_module_path() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_resolve_path");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    fs::write(temp_dir.join("mymod.py"), "").unwrap();
    fs::create_dir_all(temp_dir.join("pkg")).unwrap();
    fs::write(temp_dir.join("pkg/__init__.py"), "").unwrap();

    let mut resolver = ImportResolver::with_search_paths(vec![temp_dir.clone()]);
    resolver.build_index();

    // Resolve module
    let path = resolver.resolve_module_path("mymod");
    assert!(path.is_some());
    assert!(path.unwrap().ends_with("mymod.py"));

    // Resolve package
    let pkg_path = resolver.resolve_module_path("pkg");
    assert!(pkg_path.is_some());
    assert!(pkg_path.unwrap().ends_with("__init__.py"));

    // Non-existent module
    let none_path = resolver.resolve_module_path("nonexistent");
    assert!(none_path.is_none());

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_circular_import_detection() {
    let mut resolver = ImportResolver::new();

    // Start loading module_a
    resolver.start_loading("module_a");
    assert!(resolver.is_loading("module_a"));
    assert!(!resolver.is_loading("module_b"));

    // Start loading module_b (simulating module_a importing module_b)
    resolver.start_loading("module_b");
    assert!(resolver.is_loading("module_b"));

    // Finish loading module_b
    resolver.finish_loading("module_b");
    assert!(!resolver.is_loading("module_b"));

    // module_a is still loading
    assert!(resolver.is_loading("module_a"));

    resolver.finish_loading("module_a");
    assert!(!resolver.is_loading("module_a"));
}

#[test]
fn test_clear_resolver() {
    let mut resolver = ImportResolver::new();
    resolver.register_module("test", ModuleInfo::new("test"));
    resolver.start_loading("loading");

    resolver.clear();

    assert!(!resolver.has_module("test"));
    assert!(!resolver.is_loading("loading"));
    assert!(!resolver.is_indexed());
}

#[test]
fn test_module_names_iterator() {
    let mut resolver = ImportResolver::new();
    resolver.register_module("mod1", ModuleInfo::new("mod1"));
    resolver.register_module("mod2", ModuleInfo::new("mod2"));

    let names: Vec<_> = resolver.module_names().collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&&"mod1".to_string()));
    assert!(names.contains(&&"mod2".to_string()));
}
