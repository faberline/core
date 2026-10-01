use super::super::SymbolTableBuilder;
use crate::domain::syntax::language::Language;
use crate::infrastructure::syntax::multi_parser::MultiParser;

#[test]
fn test_go_function_and_types() {
    let source = r#"
package main

import "fmt"

// Add adds two integers.
func Add(a int, b int) int {
    return a + b
}

// Server represents an HTTP server.
type Server struct {
    Host string
    Port int
}

// Handler is the request handler interface.
type Handler interface {
    ServeHTTP(w ResponseWriter, r *Request)
}

const MaxRetries = 3

var defaultTimeout int = 30
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Go).unwrap();
    let table = SymbolTableBuilder::new().build_go(&parsed);
    let symbols = table.all_symbols();

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"main"),
        "missing package 'main', got: {:?}",
        names
    );
    assert!(
        names.contains(&"Add"),
        "missing function 'Add', got: {:?}",
        names
    );
    assert!(
        names.contains(&"Server"),
        "missing struct 'Server', got: {:?}",
        names
    );
    assert!(
        names.contains(&"Handler"),
        "missing interface 'Handler', got: {:?}",
        names
    );
    assert!(
        names.contains(&"MaxRetries"),
        "missing const 'MaxRetries', got: {:?}",
        names
    );
    assert!(
        names.contains(&"defaultTimeout"),
        "missing var 'defaultTimeout', got: {:?}",
        names
    );
    assert!(
        names.contains(&"fmt"),
        "missing import 'fmt', got: {:?}",
        names
    );

    // Check doc comment on Add
    let add_fn = symbols.iter().find(|s| s.name == "Add").unwrap();
    assert_eq!(add_fn.doc.as_deref(), Some("Add adds two integers."));

    // Check struct kind
    let server = symbols.iter().find(|s| s.name == "Server").unwrap();
    assert_eq!(server.kind, super::super::SymbolKind::Struct);

    // Check interface kind
    let handler = symbols.iter().find(|s| s.name == "Handler").unwrap();
    assert_eq!(handler.kind, super::super::SymbolKind::Interface);

    // Check const kind
    let max_retries = symbols.iter().find(|s| s.name == "MaxRetries").unwrap();
    assert_eq!(max_retries.kind, super::super::SymbolKind::Const);
}

#[test]
fn test_go_method_declaration() {
    let source = r#"
package main

type MyStruct struct {
    Value int
}

// Process does something useful.
func (s *MyStruct) Process() error {
    return nil
}
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(source, Language::Go).unwrap();
    let table = SymbolTableBuilder::new().build_go(&parsed);
    let symbols = table.all_symbols();

    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"Process"),
        "missing method 'Process', got: {:?}",
        names
    );

    let process = symbols.iter().find(|s| s.name == "Process").unwrap();
    assert_eq!(process.kind, super::super::SymbolKind::Function);
    assert_eq!(
        process.doc.as_deref(),
        Some("Process does something useful.")
    );
}

#[test]
fn test_go_type_parsing() {
    assert_eq!(
        super::parse_go_type("int"),
        super::super::TypeInfo::Primitive("int".to_string())
    );
    assert_eq!(
        super::parse_go_type("string"),
        super::super::TypeInfo::Primitive("string".to_string())
    );
}
