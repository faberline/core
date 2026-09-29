use super::*;

fn parse_rust(code: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();
    parser.parse(code, None).unwrap()
}

#[test]
fn test_extract_rust_docstrings() {
    let code = r#"
/// This is a documented function
/// with multiple lines
fn documented_fn() {}

struct UndocumentedStruct {}

/// Documented struct
struct DocStruct {
    field: i32,
}
"#;
    let tree = parse_rust(code);
    let provider = RustSearchProvider::new();
    let docs = provider.extract_docstrings(&tree.root_node(), code);

    assert!(docs.contains_key("documented_fn"));
    assert!(docs
        .get("documented_fn")
        .unwrap()
        .contains("documented function"));
    assert!(docs.contains_key("DocStruct"));
    assert!(!docs.contains_key("UndocumentedStruct"));
}

#[test]
fn test_rust_call_graph() {
    let code = r#"
fn caller() {
    callee();
    helper();
}

fn callee() {}
fn helper() {}
"#;
    let tree = parse_rust(code);
    let provider = RustSearchProvider::new();
    let file = PathBuf::from("test.rs");
    let calls = provider.build_call_graph(&tree.root_node(), code, &file);

    assert_eq!(calls.len(), 2);
    assert!(calls
        .iter()
        .any(|c| c.caller == "caller" && c.callee == "callee"));
    assert!(calls
        .iter()
        .any(|c| c.caller == "caller" && c.callee == "helper"));
}

#[test]
fn test_find_rust_usages() {
    let code = r#"
fn foo() {
    let x = bar();
    bar();
}

fn bar() -> i32 { 42 }
"#;
    let tree = parse_rust(code);
    let provider = RustSearchProvider::new();
    let file = PathBuf::from("test.rs");
    let usages = provider.find_usages(&tree.root_node(), code, "bar", &file);

    // Should find: definition, two calls
    assert_eq!(usages.len(), 3);
}

#[test]
fn test_find_trait_implementations() {
    let code = r#"
trait MyTrait {
    fn method(&self);
}

struct Foo;

impl MyTrait for Foo {
    fn method(&self) {}
}

struct Bar;

impl MyTrait for Bar {
    fn method(&self) {}
}

impl Clone for Foo {
    fn clone(&self) -> Self { Foo }
}
"#;
    let tree = parse_rust(code);
    let provider = RustSearchProvider::new();
    let file = PathBuf::from("test.rs");
    let impls = provider.find_implementations(&tree.root_node(), code, "MyTrait", &file);

    assert_eq!(impls.len(), 2);
    let symbols: Vec<_> = impls.iter().filter_map(|m| m.symbol.clone()).collect();
    assert!(symbols.contains(&"Foo".to_string()));
    assert!(symbols.contains(&"Bar".to_string()));
}
