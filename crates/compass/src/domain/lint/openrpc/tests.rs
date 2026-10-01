use super::*;
use crate::domain::lint::checker::Checker;

fn check(source: &str) -> Vec<Diagnostic> {
    let file = ParsedFile::line_based(source.to_string(), Language::Yaml);
    OpenRpcChecker::new().check(&file, &LintConfig::default())
}

#[test]
fn test_is_openrpc_detects() {
    assert!(OpenRpcChecker::is_openrpc(
        "{\"openrpc\": \"1.2.6\", \"methods\": []}"
    ));
    assert!(!OpenRpcChecker::is_openrpc("{\"openapi\": \"3.0.0\"}"));
    assert!(!OpenRpcChecker::is_openrpc("asyncapi: 2.0.0\n"));
}

#[test]
fn test_missing_required_fields() {
    let source = "{\n  \"openrpc\": \"1.2.6\"\n}\n";
    let diags = check(source);
    let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"OR001"), "expected OR001, got {:?}", codes);
}

#[test]
fn test_valid_openrpc_no_false_positives() {
    let source = r#"{
  "openrpc": "1.2.6",
  "info": {
    "title": "My RPC API",
    "version": "1.0.0"
  },
  "methods": [
    {
      "name": "getUser",
      "params": [],
      "result": { "name": "user", "schema": {} }
    }
  ]
}
"#;
    let diags = check(source);
    let or001: Vec<_> = diags.iter().filter(|d| d.code == "OR001").collect();
    let or002: Vec<_> = diags.iter().filter(|d| d.code == "OR002").collect();
    let or003: Vec<_> = diags.iter().filter(|d| d.code == "OR003").collect();
    let or004: Vec<_> = diags.iter().filter(|d| d.code == "OR004").collect();
    assert!(or001.is_empty(), "unexpected OR001: {:?}", or001);
    assert!(or002.is_empty(), "unexpected OR002: {:?}", or002);
    assert!(or003.is_empty(), "unexpected OR003: {:?}", or003);
    assert!(or004.is_empty(), "unexpected OR004: {:?}", or004);
}

#[test]
fn test_method_missing_params_and_result() {
    let source = r#"{
  "openrpc": "1.2.6",
  "info": { "title": "T", "version": "1.0.0" },
  "methods": [
    {
      "name": "doSomething"
    }
  ]
}
"#;
    let diags = check(source);
    assert!(
        diags.iter().any(|d| d.code == "OR002"),
        "expected OR002, got {:?}",
        diags
    );
    assert!(
        diags.iter().any(|d| d.code == "OR003"),
        "expected OR003, got {:?}",
        diags
    );
}

#[test]
fn test_duplicate_method_names() {
    let source = r#"{
  "openrpc": "1.2.6",
  "info": { "title": "T", "version": "1.0.0" },
  "methods": [
    { "name": "getUser", "params": [], "result": {} },
    { "name": "getUser", "params": [], "result": {} }
  ]
}
"#;
    let diags = check(source);
    assert!(
        diags.iter().any(|d| d.code == "OR004"),
        "expected OR004, got {:?}",
        diags
    );
}

#[test]
fn test_not_openrpc_returns_empty() {
    let source = "{\n  \"openapi\": \"3.0.0\"\n}\n";
    let diags = check(source);
    assert!(diags.is_empty(), "expected no diags for non-openrpc JSON");
}
