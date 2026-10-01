use super::*;
use crate::domain::lint::checker::Checker;

fn check(source: &str) -> Vec<Diagnostic> {
    let file = ParsedFile::line_based(source.to_string(), Language::Yaml);
    OpenApiChecker::new().check(&file, &LintConfig::default())
}

#[test]
fn test_is_openapi_detects_version() {
    assert!(OpenApiChecker::is_openapi(
        "openapi: 3.0.0\ninfo:\n  title: Test\n"
    ));
    assert!(OpenApiChecker::is_openapi("openapi: \"3.1.0\"\n"));
    assert!(!OpenApiChecker::is_openapi("asyncapi: 2.0.0\n"));
    assert!(!OpenApiChecker::is_openapi("apiVersion: v1\nkind: Pod\n"));
}

#[test]
fn test_missing_required_fields() {
    let source = "openapi: 3.0.0\ninfo:\n  title: My API\n";
    let diags = check(source);
    let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"OA001"), "expected OA001, got {:?}", codes);
}

#[test]
fn test_valid_openapi_no_false_positives() {
    let source = "\
openapi: 3.0.0
info:
  title: My API
  version: 1.0.0
paths:
  /users:
    get:
      operationId: listUsers
      responses:
        '200':
          description: OK
";
    let diags = check(source);
    let oa001: Vec<_> = diags.iter().filter(|d| d.code == "OA001").collect();
    let oa003: Vec<_> = diags.iter().filter(|d| d.code == "OA003").collect();
    let oa004: Vec<_> = diags.iter().filter(|d| d.code == "OA004").collect();
    assert!(oa001.is_empty(), "unexpected OA001: {:?}", oa001);
    assert!(oa003.is_empty(), "unexpected OA003: {:?}", oa003);
    assert!(oa004.is_empty(), "unexpected OA004: {:?}", oa004);
}

#[test]
fn test_invalid_ref_format() {
    let source = "\
openapi: 3.0.0
info:
  title: T
  version: 1.0.0
paths:
  /x:
    get:
      operationId: getX
      requestBody:
        $ref: bad_ref_no_hash
";
    let diags = check(source);
    assert!(
        diags.iter().any(|d| d.code == "OA002"),
        "expected OA002, got {:?}",
        diags
    );
}

#[test]
fn test_empty_path_item() {
    let source = "\
openapi: 3.0.0
info:
  title: T
  version: 1.0.0
paths:
  /empty:
    x-custom: value
";
    let diags = check(source);
    assert!(
        diags.iter().any(|d| d.code == "OA003"),
        "expected OA003, got {:?}",
        diags
    );
}

#[test]
fn test_missing_operation_id() {
    let source = "\
openapi: 3.0.0
info:
  title: T
  version: 1.0.0
paths:
  /items:
    get:
      responses:
        '200':
          description: OK
";
    let diags = check(source);
    assert!(
        diags.iter().any(|d| d.code == "OA004"),
        "expected OA004, got {:?}",
        diags
    );
}

#[test]
fn test_not_openapi_returns_empty() {
    let source = "kind: Pod\napiVersion: v1\n";
    let diags = check(source);
    assert!(diags.is_empty(), "expected no diags for non-openapi YAML");
}
