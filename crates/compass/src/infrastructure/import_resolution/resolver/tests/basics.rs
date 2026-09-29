use super::*;

#[test]
fn test_module_registration() {
    let mut resolver = ImportResolver::new();

    let mut module_info = ModuleInfo::new("mymodule");
    module_info.exports.insert(
        "MyClass".to_string(),
        Type::Instance {
            name: "MyClass".to_string(),
            module: Some("mymodule".to_string()),
            type_args: vec![],
        },
    );

    resolver.register_module("mymodule", module_info);

    let module = resolver.get_module("mymodule");
    assert!(module.is_some());
    assert!(module.unwrap().exports.contains_key("MyClass"));
}

#[test]
fn test_from_import_resolution() {
    let mut resolver = ImportResolver::new();

    let mut module_info = ModuleInfo::new("mymodule");
    module_info.exports.insert("foo".to_string(), Type::Int);
    module_info.exports.insert("bar".to_string(), Type::Str);

    resolver.register_module("mymodule", module_info);

    let import = Import::FromModule {
        module: "mymodule".to_string(),
        names: vec![
            ImportedName {
                name: "foo".to_string(),
                alias: None,
            },
            ImportedName {
                name: "bar".to_string(),
                alias: Some("baz".to_string()),
            },
        ],
    };

    let resolved = resolver.resolve_import(&import);
    assert_eq!(resolved.get("foo"), Some(&Type::Int));
    assert_eq!(resolved.get("baz"), Some(&Type::Str));
    assert!(!resolved.contains_key("bar")); // aliased, not available as "bar"
}

#[test]
fn test_builtins_module() {
    let builtins = create_builtins_module();
    assert!(builtins.exports.contains_key("int"));
    assert!(builtins.exports.contains_key("str"));
    assert!(builtins.exports.contains_key("len"));
}

#[test]
fn test_typing_module() {
    let typing = create_typing_module();
    assert!(typing.exports.contains_key("List"));
    assert!(typing.exports.contains_key("Optional"));
    assert!(typing.exports.contains_key("TypeVar"));
}

#[test]
fn test_module_info_from_file() {
    let info = ModuleInfo::from_file("mymodule", PathBuf::from("src/mymodule.py"));
    assert_eq!(info.path, "mymodule");
    assert!(!info.is_stub);
    assert!(!info.is_package);

    let stub_info = ModuleInfo::from_file("mymodule", PathBuf::from("src/mymodule.pyi"));
    assert!(stub_info.is_stub);

    let package_info =
        ModuleInfo::from_file("mypackage", PathBuf::from("src/mypackage/__init__.py"));
    assert!(package_info.is_package);
}

#[test]
fn test_with_search_paths() {
    let paths = vec![PathBuf::from("/lib1"), PathBuf::from("/lib2")];
    let resolver = ImportResolver::with_search_paths(paths.clone());
    assert_eq!(resolver.search_paths(), &paths);
}

#[test]
fn test_add_search_path() {
    let mut resolver = ImportResolver::new();
    resolver.add_search_path(PathBuf::from("/lib1"));
    resolver.add_search_path(PathBuf::from("/lib2"));
    resolver.add_search_path(PathBuf::from("/lib1")); // Duplicate

    assert_eq!(resolver.search_paths().len(), 2);
}
