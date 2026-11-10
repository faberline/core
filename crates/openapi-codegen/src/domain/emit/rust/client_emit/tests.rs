use super::*;
use crate::build_type_map;
use crate::ir::openapi::Spec;
use crate::ir::operations;

fn render() -> String {
    let s: Spec = serde_json::from_str(r##"{"paths":{}}"##).unwrap();
    let tm = build_type_map(&s);
    emit(&operations::build(&s), &tm, None)
}

#[test]
fn a_private_ca_replaces_the_public_roots_rather_than_joining_them() {
    let out = render();
    assert!(
        out.contains(".tls_built_in_root_certs(false)"),
        "keeping the public roots means any public CA can still vouch for this \
         name, which is the thing a private trust domain exists to prevent"
    );
    assert!(out.contains(".add_root_certificate(anchor)"));
}

#[test]
fn the_verified_name_must_be_the_addressed_name() {
    let out = render();
    assert!(out.contains("if addressed != trust.server_name {"));
    assert!(out.contains("fn addressed_host(base_url: &str) -> &str {"));
}

#[test]
fn no_generated_client_offers_to_skip_verification() {
    let out = render();
    for weakening in [
        "danger_accept_invalid_certs",
        "danger_accept_invalid_hostnames",
        "insecure",
        "skip_verify",
    ] {
        assert!(
            !out.contains(weakening),
            "generated clients must not ship {weakening}: the fix for an unverifiable \
             server is the anchor or the name, not the check"
        );
    }
}

fn render_spec(json: &str) -> String {
    let s: Spec = serde_json::from_str(json).unwrap();
    let tm = build_type_map(&s);
    emit(&operations::build(&s), &tm, None)
}

const GET_TASK_SPEC: &str = r##"{
  "paths": {
    "/tasks/{id}": {
      "get": {
        "operationId": "getTask",
        "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "string"}}],
        "responses": {
          "200": {"description": "ok", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Task"}}}},
          "404": {"description": "missing", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ErrorEnvelope"}}}},
          "4XX": {"description": "client", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ErrorEnvelope"}}}},
          "default": {"description": "other"}
        }
      }
    }
  },
  "components": {"schemas": {
    "Task": {"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]},
    "ErrorEnvelope": {"type": "object", "properties": {"error": {"type": "string"}, "message": {"type": "string"}}, "required": ["error", "message"]}
  }}
}"##;

#[test]
fn declared_errors_get_a_typed_enum_and_apierror_result() {
    let out = render_spec(GET_TASK_SPEC);
    assert!(out.contains("pub enum GetTaskError {"), "{out}");
    assert!(out.contains("Status404(ErrorEnvelope),"), "{out}");
    assert!(out.contains("Status4XX(ErrorEnvelope),"), "{out}");
    assert!(out.contains("Default(String),"), "{out}");
    assert!(out.contains("pub enum ApiError<E> {"), "{out}");
    assert!(out.contains("-> ApiResult<Task, GetTaskError> {"), "{out}");
}

#[test]
fn declared_errors_dispatch_by_status() {
    let out = render_spec(GET_TASK_SPEC);
    assert!(
        out.contains("404 => Err(ApiError::Api(GetTaskError::Status404(resp.json()?))),"),
        "{out}"
    );
    assert!(
        out.contains("400..=499 => Err(ApiError::Api(GetTaskError::Status4XX(resp.json()?))),"),
        "{out}"
    );
    assert!(
        out.contains("_ => Err(ApiError::Api(GetTaskError::Default(resp.text()?))),"),
        "{out}"
    );
}

#[test]
fn an_operation_without_declared_errors_keeps_client_result() {
    let out = render_spec(
        r##"{"paths":{"/ping":{"get":{"operationId":"ping",
          "responses":{"200":{"description":"ok"}}}}}}"##,
    );
    assert!(out.contains("-> ClientResult<()> {"), "{out}");
    assert!(!out.contains("ApiError"), "{out}");
    assert!(!out.contains("ApiResult"), "{out}");
}

#[test]
fn an_undeclared_status_falls_back_to_error_for_status() {
    let out = render_spec(
        r##"{"paths":{"/tasks/{id}":{"get":{"operationId":"getTask",
          "parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string"}}],
          "responses":{
            "200":{"description":"ok","content":{"application/json":{"schema":{"type":"object"}}}},
            "404":{"description":"missing","content":{"application/json":{"schema":{"type":"object"}}}}}}}}}"##,
    );
    assert!(out.contains("expect_err(\"4xx or 5xx status\")"), "{out}");
    assert!(!out.contains("Default("), "{out}");
}
