use super::super::SymbolTableBuilder;
use crate::domain::syntax::language::Language;
use crate::infrastructure::syntax::multi_parser::MultiParser;

#[test]
fn test_rust_struct_and_impl() {
    let source = r#"
/// A simple struct
struct MyStruct {
    value: i32,
}

impl MyStruct {
    /// Creates a new instance
    fn new(value: i32) -> Self {
        MyStruct { value }
    }

    fn process(&self) -> i32 {
        self.value * 2
    }
}
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Rust).unwrap();
    let table = SymbolTableBuilder::new().build_rust(&parsed);
    let symbols = table.all_symbols();

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"MyStruct"),
        "missing MyStruct, got: {:?}",
        names
    );
    assert!(names.contains(&"value"), "missing field 'value'");
    assert!(names.contains(&"new"), "missing method 'new'");
    assert!(names.contains(&"process"), "missing method 'process'");

    // Check doc comment on struct
    let my_struct = symbols.iter().find(|s| s.name == "MyStruct").unwrap();
    assert_eq!(my_struct.doc.as_deref(), Some("A simple struct"));

    // Check doc comment on new()
    let new_fn = symbols.iter().find(|s| s.name == "new").unwrap();
    assert_eq!(new_fn.doc.as_deref(), Some("Creates a new instance"));
}

#[test]
fn test_rust_function_with_return_type() {
    let source = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Rust).unwrap();
    let table = SymbolTableBuilder::new().build_rust(&parsed);
    let symbols = table.all_symbols();

    let add_fn = symbols.iter().find(|s| s.name == "add").unwrap();
    assert!(add_fn.type_info.is_some());

    // Check parameters
    let params: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == super::super::SymbolKind::Parameter && s.name != "self")
        .map(|s| s.name.as_str())
        .collect();
    assert!(
        params.contains(&"a"),
        "missing param 'a', got: {:?}",
        params
    );
    assert!(
        params.contains(&"b"),
        "missing param 'b', got: {:?}",
        params
    );
}

#[test]
fn test_rust_trait() {
    let source = r#"
/// A display trait
trait Displayable {
    fn display(&self) -> String;
}
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Rust).unwrap();
    let table = SymbolTableBuilder::new().build_rust(&parsed);
    let symbols = table.all_symbols();

    let trait_sym = symbols.iter().find(|s| s.name == "Displayable").unwrap();
    assert_eq!(trait_sym.kind, super::super::SymbolKind::Trait);
    assert_eq!(trait_sym.doc.as_deref(), Some("A display trait"));

    let display_fn = symbols.iter().find(|s| s.name == "display");
    assert!(
        display_fn.is_some(),
        "missing 'display' method in trait, got: {:?}",
        symbols
            .iter()
            .map(|s| (&s.name, s.kind))
            .collect::<Vec<_>>()
    );
    assert_eq!(display_fn.unwrap().kind, super::super::SymbolKind::Function);
}

#[test]
fn test_rust_enum() {
    let source = r#"
enum Color {
    Red,
    Green,
    Blue,
}
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Rust).unwrap();
    let table = SymbolTableBuilder::new().build_rust(&parsed);
    let symbols = table.all_symbols();

    let enum_sym = symbols.iter().find(|s| s.name == "Color").unwrap();
    assert_eq!(enum_sym.kind, super::super::SymbolKind::Enum);

    let variants: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == super::super::SymbolKind::EnumMember)
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(variants.len(), 3);
    assert!(variants.contains(&"Red"));
    assert!(variants.contains(&"Green"));
    assert!(variants.contains(&"Blue"));
}

#[test]
fn test_rust_const_and_static() {
    let source = r#"
const MAX_SIZE: usize = 100;
static COUNTER: i32 = 0;
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Rust).unwrap();
    let table = SymbolTableBuilder::new().build_rust(&parsed);
    let symbols = table.all_symbols();

    let max_size = symbols.iter().find(|s| s.name == "MAX_SIZE").unwrap();
    assert_eq!(max_size.kind, super::super::SymbolKind::Const);
    assert!(max_size.type_info.is_some());

    let counter = symbols.iter().find(|s| s.name == "COUNTER").unwrap();
    assert_eq!(counter.kind, super::super::SymbolKind::Static);
}

#[test]
fn test_rust_mod_and_use() {
    let source = r#"
mod utils;
use std::collections::HashMap;
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Rust).unwrap();
    let table = SymbolTableBuilder::new().build_rust(&parsed);
    let symbols = table.all_symbols();

    let mod_sym = symbols.iter().find(|s| s.name == "utils").unwrap();
    assert_eq!(mod_sym.kind, super::super::SymbolKind::Module);

    let import_sym = symbols.iter().find(|s| s.name == "HashMap").unwrap();
    assert_eq!(import_sym.kind, super::super::SymbolKind::Import);
}

#[test]
fn test_rust_macro_definition() {
    let source = r#"
macro_rules! my_macro {
    () => {};
}
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Rust).unwrap();
    let table = SymbolTableBuilder::new().build_rust(&parsed);
    let symbols = table.all_symbols();

    let mac = symbols.iter().find(|s| s.name == "my_macro").unwrap();
    assert_eq!(mac.kind, super::super::SymbolKind::Macro);
}

#[test]
fn test_rust_error_recovery() {
    let source = r#"
fn valid_fn() -> i32 {
    42
}

fn broken( {
    0
}

struct ValidStruct {
    x: i32,
}
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Rust).unwrap();
    let table = SymbolTableBuilder::new().build_rust(&parsed);
    let symbols = table.all_symbols();

    // Should still extract valid symbols despite parse errors
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"valid_fn"),
        "missing valid_fn, got: {:?}",
        names
    );
    assert!(
        names.contains(&"ValidStruct"),
        "missing ValidStruct, got: {:?}",
        names
    );
}

#[test]
fn test_rust_impl_trait_for_type() {
    let source = r#"
struct Foo;

trait Bar {
    fn do_thing(&self);
}

impl Bar for Foo {
    fn do_thing(&self) {}
}
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Rust).unwrap();
    let table = SymbolTableBuilder::new().build_rust(&parsed);
    let symbols = table.all_symbols();

    let impl_sym = symbols
        .iter()
        .find(|s| s.kind == super::super::SymbolKind::Impl)
        .unwrap();
    assert_eq!(impl_sym.name, "Bar for Foo");
}

#[test]
fn test_rust_scoping() {
    let source = r#"
fn outer() {
    fn inner() {}
}
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Rust).unwrap();
    let table = SymbolTableBuilder::new().build_rust(&parsed);
    let symbols = table.all_symbols();

    let outer = symbols.iter().find(|s| s.name == "outer").unwrap();
    let inner = symbols.iter().find(|s| s.name == "inner").unwrap();
    assert_ne!(
        outer.scope_id, inner.scope_id,
        "inner should be in a different scope"
    );
}
