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

#[test]
fn ndjson_operation_is_emitted_as_stream() {
    let out = render_spec(
        r##"{"paths":{"/logs":{"post":{"operationId":"uploadLogs",
  "requestBody":{"required":true,"content":{"application/x-ndjson":{"schema":{"type":"object"}}}},
  "responses":{"200":{"description":"ok","content":{"application/x-ndjson":{"schema":{"type":"string"}}}}}}}}}"##,
    );
    let sig_start = out.find("fn upload_logs(").expect("method is emitted");
    let sig_end = out[sig_start..].find('{').unwrap() + sig_start;
    let sig = &out[sig_start..sig_end];
    assert!(sig.contains("body: impl Iterator<Item = "), "{sig}");
    assert!(
        sig.ends_with("-> ClientResult<NdjsonRecords<String>> "),
        "{sig}"
    );
}

const NDJSON_ITEM: &str = r##""components":{"schemas":{"Item":{"type":"object","properties":{"id":{"type":"integer"}},"required":["id"]}}}"##;

#[test]
fn ndjson_upload_method_takes_an_iterator() {
    let out = render_spec(&format!(
        r##"{{"paths":{{"/items":{{"post":{{"operationId":"uploadItems",
  "requestBody":{{"required":true,"content":{{"application/x-ndjson":{{"schema":{{"$ref":"#/components/schemas/Item"}}}}}}}},
  "responses":{{"200":{{"description":"ok","content":{{"application/json":{{"schema":{{"$ref":"#/components/schemas/Item"}}}}}}}}}}}}}}}},{NDJSON_ITEM}}}"##
    ));
    assert!(
        out.contains("pub fn upload_items(&self, body: impl Iterator<Item = Item> + Send + 'static) -> ClientResult<Item> {"),
        "{out}"
    );
    assert!(
        out.contains("req = req.header(\"Content-Type\", \"application/x-ndjson\").body(reqwest::blocking::Body::new(NdjsonBodyReader::new(body)));"),
        "{out}"
    );
    assert!(
        out.contains("struct NdjsonBodyReader<I: Iterator> {"),
        "{out}"
    );
}

#[test]
fn ndjson_response_method_returns_a_record_iterator() {
    let out = render_spec(&format!(
        r##"{{"paths":{{"/items":{{"get":{{"operationId":"listItems",
  "responses":{{"200":{{"description":"ok","content":{{"application/x-ndjson":{{"schema":{{"$ref":"#/components/schemas/Item"}}}}}}}}}}}}}}}},{NDJSON_ITEM}}}"##
    ));
    assert!(
        out.contains("pub fn list_items(&self) -> ClientResult<NdjsonRecords<Item>> {"),
        "{out}"
    );
    assert!(out.contains("let slot = self.acquire_slot()?;"), "{out}");
    assert!(out.contains("Ok(NdjsonRecords::new(resp, slot))"), "{out}");
    assert!(
        out.contains("req = req.header(\"Accept\", \"application/x-ndjson\");"),
        "{out}"
    );
    assert!(out.contains("pub struct NdjsonRecords<T> {"), "{out}");
}

#[test]
fn ndjson_adds_a_typed_parse_error() {
    let out = render_spec(&format!(
        r##"{{"paths":{{"/items":{{"get":{{"operationId":"listItems",
  "responses":{{"200":{{"description":"ok","content":{{"application/x-ndjson":{{"schema":{{"$ref":"#/components/schemas/Item"}}}}}}}}}}}}}}}},{NDJSON_ITEM}}}"##
    ));
    assert!(
        out.contains("Ndjson { line: usize, reason: String },"),
        "{out}"
    );
    assert!(
        out.contains("Self::Trust(_) | Self::Ndjson { .. } => None,"),
        "{out}"
    );
}

#[test]
fn json_only_client_has_no_ndjson_output() {
    let out = render_spec(
        r##"{"paths":{"/pets":{"get":{"operationId":"listPets","responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"type":"array","items":{"type":"string"}}}}}}}}}}"##,
    );
    assert!(!out.contains("Ndjson"), "{out}");
    assert!(!out.contains("NdjsonRecords"), "{out}");
    assert!(!out.contains("NdjsonBodyReader"), "{out}");
}

fn managed_out() -> String {
    let s: Spec = serde_json::from_str(
        r##"{"paths":{"/ping":{"get":{"operationId":"ping","responses":{"204":{"description":"ok"}}}}}}"##,
    )
    .unwrap();
    let tm = build_type_map(&s);
    let managed =
        crate::domain::ManagedAuth::new("/etc/ca/ca.crt", [crate::domain::FileBearerScheme::Https])
            .unwrap();
    emit_with(&operations::build(&s), &tm, None, Some(&managed))
}

#[test]
fn managed_client_takes_a_ca_path_input() {
    let out = managed_out();
    assert!(
        out.contains("const MANAGED_CA_PATH: &str = \"/etc/ca/ca.crt\";"),
        "{out}"
    );
    assert!(
        out.contains("pub fn with_ca_path(base_url: impl Into<String>, policy: TransportPolicy, ca_path: impl Into<std::path::PathBuf>) -> ClientResult<Self>"),
        "{out}"
    );
    assert!(
        out.contains("Self::with_ca_path(base_url, policy, MANAGED_CA_PATH)"),
        "{out}"
    );
}

#[test]
fn managed_client_refuses_non_https_before_transport() {
    let out = managed_out();
    assert!(
        out.contains("managed clients accept only https: URLs"),
        "{out}"
    );
    assert!(out.contains(".https_only(true)"), "{out}");
    let guard = out
        .find("attach_managed(&self.base_url, &self.ca_path, &mut request_headers)?;")
        .expect("managed method attaches the token before sending");
    let slot = out
        .find("let _slot = self.acquire_slot()?;")
        .expect("method acquires a slot");
    let send = out.find("req.send()").expect("method sends");
    assert!(guard < slot, "{out}");
    assert!(guard < send, "{out}");
}

#[test]
fn managed_client_uses_the_fixed_token_path() {
    let out = managed_out();
    assert!(
        out.contains("const MANAGED_TOKEN_PATH: &str = \"/var/run/secrets/kubernetes.io/serviceaccount/token\";"),
        "{out}"
    );
    assert!(
        out.contains("managed_read(std::path::Path::new(MANAGED_TOKEN_PATH), \"token\")?"),
        "{out}"
    );
    assert!(!out.contains("FILE_BEARER"), "{out}");
    assert!(!out.contains("attach_file_bearer"), "{out}");
}

#[test]
fn managed_client_fails_before_transport_on_an_empty_token_or_ca() {
    let out = managed_out();
    assert!(out.contains("managed {what} is unavailable"), "{out}");
    assert!(out.contains("managed {what} is empty"), "{out}");
    assert!(
        out.contains("managed_read(ca_path, \"CA bundle\")?"),
        "{out}"
    );
    assert!(
        out.contains("managed_read(&ca_path, \"CA bundle\")?"),
        "{out}"
    );
}

#[test]
fn managed_client_never_retries_without_the_token() {
    let out = managed_out();
    assert_eq!(out.matches("req.send()").count(), 1, "{out}");
    assert!(
        out.contains("headers.insert(AUTHORIZATION, value);"),
        "{out}"
    );
    assert!(out.contains("value.set_sensitive(true);"), "{out}");
    let refusal = out.find("accept only https").expect("https check exists");
    let token_read = out
        .find("std::path::Path::new(MANAGED_TOKEN_PATH)")
        .expect("token read exists");
    assert!(refusal < token_read, "{out}");
}

#[test]
fn managed_client_trusts_only_the_ca_and_has_no_public_fallback() {
    let out = managed_out();
    assert!(out.contains(".tls_built_in_root_certs(false)"), "{out}");
    assert!(out.contains(".add_root_certificate(anchor)"), "{out}");
    // The shared `PrivateTrust` doc comment still names `with_private_ca`; the
    // constructor itself must be absent.
    assert!(!out.contains("pub fn with_private_ca"), "{out}");
    assert!(!out.contains("danger_accept_invalid_certs"), "{out}");
    assert!(!out.contains("danger_accept_invalid_hostnames"), "{out}");
}

#[test]
fn non_managed_client_is_unchanged() {
    let s: Spec = serde_json::from_str(
        r##"{"paths":{"/ping":{"get":{"operationId":"ping","responses":{"204":{"description":"ok"}}}}}}"##,
    )
    .unwrap();
    let tm = build_type_map(&s);
    let ops = operations::build(&s);
    let plain = emit(&ops, &tm, None);
    assert_eq!(plain, emit_with(&ops, &tm, None, None));
    assert!(plain.contains("pub fn new(base_url: impl Into<String>) -> Self"));
    assert!(plain.contains("with_private_ca"));
    for needle in ["MANAGED_", "attach_managed", "ca_path", "Managed("] {
        assert!(!plain.contains(needle), "{needle} leaked into {plain}");
    }
}
