use super::*;

#[test]
fn test_stub_loader_builtins() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    assert!(loader.has_stub("builtins"));
    assert!(loader.has_stub("typing"));

    let builtins = loader.get_stub("builtins").unwrap();
    assert!(builtins.exports.contains_key("int"));
    assert!(builtins.exports.contains_key("str"));
    assert!(builtins.exports.contains_key("len"));
    assert!(builtins.exports.contains_key("Exception"));
}

#[test]
fn test_typing_stub() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    let typing = loader.get_stub("typing").unwrap();
    assert!(typing.exports.contains_key("List"));
    assert!(typing.exports.contains_key("Optional"));
    assert!(typing.exports.contains_key("TypeVar"));
    assert!(typing.exports.contains_key("Protocol"));
    assert!(typing.exports.contains_key("Any"));
}

#[test]
fn test_collections_stub() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    let collections = loader.get_stub("collections").unwrap();
    assert!(collections.exports.contains_key("deque"));
    assert!(collections.exports.contains_key("defaultdict"));
    assert!(collections.exports.contains_key("Counter"));
}

#[test]
fn test_stub_path_management() {
    let mut loader = StubLoader::new();

    loader.add_stub_path(PathBuf::from("/usr/lib/python3/stubs"));
    loader.add_stub_path(PathBuf::from("/home/user/.stubs"));

    assert_eq!(loader.stub_paths.len(), 2);

    // Adding same path again shouldn't duplicate
    loader.add_stub_path(PathBuf::from("/usr/lib/python3/stubs"));
    assert_eq!(loader.stub_paths.len(), 2);
}

// Phase C tests: Typeshed integration

#[test]
fn test_typeshed_os_stub() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    let os = loader.get_stub("os").unwrap();
    assert!(os.exports.contains_key("getcwd"));
    assert!(os.exports.contains_key("getenv"));
    assert!(os.exports.contains_key("listdir"));
    assert!(os.exports.contains_key("makedirs"));
    assert!(os.exports.contains_key("remove"));
}

#[test]
fn test_typeshed_os_path_stub() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    let os_path = loader.get_stub("os.path").unwrap();
    assert!(os_path.exports.contains_key("exists"));
    assert!(os_path.exports.contains_key("join"));
    assert!(os_path.exports.contains_key("dirname"));
    assert!(os_path.exports.contains_key("basename"));
    assert!(os_path.exports.contains_key("isfile"));
    assert!(os_path.exports.contains_key("isdir"));
}

#[test]
fn test_typeshed_sys_stub() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    let sys = loader.get_stub("sys").unwrap();
    assert!(sys.exports.contains_key("argv"));
    assert!(sys.exports.contains_key("path"));
    assert!(sys.exports.contains_key("version"));
    assert!(sys.exports.contains_key("exit"));
}

#[test]
fn test_typeshed_io_stub() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    let io = loader.get_stub("io").unwrap();
    assert!(io.exports.contains_key("StringIO"));
    assert!(io.exports.contains_key("BytesIO"));
    assert!(io.exports.contains_key("open"));
}

#[test]
fn test_typeshed_json_stub() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    let json = loader.get_stub("json").unwrap();
    assert!(json.exports.contains_key("loads"));
    assert!(json.exports.contains_key("dumps"));
    assert!(json.exports.contains_key("load"));
    assert!(json.exports.contains_key("dump"));
}

#[test]
fn test_typeshed_pathlib_stub() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    let pathlib = loader.get_stub("pathlib").unwrap();
    assert!(pathlib.exports.contains_key("Path"));
    assert!(pathlib.exports.contains_key("PurePath"));
}

#[test]
fn test_get_or_load_stub() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    // Should return existing stub
    let builtins = loader.get_or_load_stub("builtins");
    assert!(builtins.is_some());
    assert!(builtins.unwrap().exports.contains_key("int"));

    // Should return None for non-existent module (no stub paths configured)
    let nonexistent = loader.get_or_load_stub("nonexistent_module");
    assert!(nonexistent.is_none());

    // Should return typeshed stubs
    let os = loader.get_or_load_stub("os");
    assert!(os.is_some());
    assert!(os.unwrap().exports.contains_key("getcwd"));
}

#[test]
fn test_all_typeshed_modules_loaded() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    // Verify all expected typeshed modules are available
    let expected_modules = [
        "builtins",
        "typing",
        "collections",
        "collections.abc",
        "os",
        "os.path",
        "sys",
        "io",
        "re",
        "json",
        "pathlib",
        "functools",
        "itertools",
        "datetime",
    ];

    for module in expected_modules {
        assert!(
            loader.has_stub(module),
            "Expected module '{}' to be available",
            module
        );
    }
}

#[test]
fn test_typing_advanced_features() {
    let mut loader = StubLoader::new();
    loader.load_builtins();

    let typing = loader.get_stub("typing").unwrap();

    // PEP 612: ParamSpec
    assert!(typing.exports.contains_key("ParamSpec"));
    assert!(typing.exports.contains_key("Concatenate"));

    // PEP 646: TypeVarTuple
    assert!(typing.exports.contains_key("TypeVarTuple"));
    assert!(typing.exports.contains_key("Unpack"));

    // PEP 675: LiteralString
    assert!(typing.exports.contains_key("LiteralString"));

    // PEP 647: TypeGuard
    assert!(typing.exports.contains_key("TypeGuard"));

    // PEP 742: TypeIs
    assert!(typing.exports.contains_key("TypeIs"));

    // Self type
    assert!(typing.exports.contains_key("Self"));

    // Final and Annotated
    assert!(typing.exports.contains_key("Final"));
    assert!(typing.exports.contains_key("Annotated"));
}
