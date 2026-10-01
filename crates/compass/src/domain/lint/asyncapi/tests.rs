use super::*;
use crate::domain::lint::checker::Checker;

fn check(source: &str) -> Vec<Diagnostic> {
    let file = ParsedFile::line_based(source.to_string(), Language::Yaml);
    AsyncApiChecker::new().check(&file, &LintConfig::default())
}

#[test]
fn test_is_asyncapi_detection() {
    assert!(AsyncApiChecker::is_asyncapi("asyncapi: 2.6.0\ninfo:\n"));
    assert!(AsyncApiChecker::is_asyncapi("asyncapi: \"3.0.0\"\n"));
    assert!(!AsyncApiChecker::is_asyncapi("openapi: 3.0.0\n"));
    assert!(!AsyncApiChecker::is_asyncapi("apiVersion: v1\n"));
}

#[test]
fn test_missing_required_fields() {
    let source = "asyncapi: 2.6.0\ninfo:\n  title: My API\n";
    let diags = check(source);
    let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"AA001"), "expected AA001, got {:?}", codes);
}

#[test]
fn test_valid_asyncapi_no_false_positives() {
    let source = "\
asyncapi: 2.6.0
info:
  title: My Event API
  version: 1.0.0
channels:
  /user/created:
    subscribe:
      message:
        payload:
          type: object
";
    let diags = check(source);
    let aa001: Vec<_> = diags.iter().filter(|d| d.code == "AA001").collect();
    let aa002: Vec<_> = diags.iter().filter(|d| d.code == "AA002").collect();
    let aa003: Vec<_> = diags.iter().filter(|d| d.code == "AA003").collect();
    assert!(aa001.is_empty(), "unexpected AA001: {:?}", aa001);
    assert!(aa002.is_empty(), "unexpected AA002: {:?}", aa002);
    assert!(aa003.is_empty(), "unexpected AA003: {:?}", aa003);
}

#[test]
fn test_channel_name_missing_slash() {
    let source = "\
asyncapi: 2.6.0
info:
  title: T
  version: 1.0.0
channels:
  userCreated:
    subscribe:
      message:
        payload:
          type: object
";
    let diags = check(source);
    assert!(
        diags.iter().any(|d| d.code == "AA002"),
        "expected AA002, got {:?}",
        diags
    );
}

#[test]
fn test_message_without_payload() {
    let source = "\
asyncapi: 2.6.0
info:
  title: T
  version: 1.0.0
channels:
  /events:
    subscribe:
      message:
        summary: An event
";
    let diags = check(source);
    assert!(
        diags.iter().any(|d| d.code == "AA003"),
        "expected AA003, got {:?}",
        diags
    );
}

#[test]
fn test_server_missing_protocol() {
    let source = "\
asyncapi: 2.6.0
info:
  title: T
  version: 1.0.0
channels:
  /x:
    subscribe:
      message:
        payload:
          type: object
servers:
  production:
    url: mqtt://example.com
";
    let diags = check(source);
    assert!(
        diags.iter().any(|d| d.code == "AA004"),
        "expected AA004, got {:?}",
        diags
    );
}

#[test]
fn test_not_asyncapi_returns_empty() {
    let source = "openapi: 3.0.0\ninfo:\n  title: T\n  version: 1.0.0\npaths: {}\n";
    let diags = check(source);
    assert!(diags.is_empty(), "expected no diags for non-asyncapi YAML");
}
