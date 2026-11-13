use super::*;
use crate::ir::openapi::Spec;
use crate::ir::operations;
use crate::{build_type_map, FileBearerScheme, ManagedAuth, PythonTarget};

fn spec(json: &str) -> Spec {
    serde_json::from_str(json).unwrap()
}

#[test]
fn init_signatures_accept_use_post_fallback() {
    let s = spec(r##"{"paths":{}}"##);
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    assert!(out.contains("use_post_fallback: bool = False"));
    assert!(out.contains("self._use_post_fallback = use_post_fallback"));
    // Both Client and AsyncClient carry the flag.
    assert_eq!(out.matches("use_post_fallback: bool = False").count(), 2);
}

#[test]
fn a_private_ca_is_the_only_anchor_and_the_name_must_match_the_base_url() {
    let s = spec(r##"{"paths":{}}"##);
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    // A cafile makes create_default_context skip the system store entirely,
    // which is the point: a private domain merged into the public one still
    // lets any public CA certify this name.
    assert!(out.contains("ssl.create_default_context(cafile=trust.ca_bundle)"));
    assert!(out.contains("context.check_hostname = True"));
    assert!(out.contains("context.verify_mode = ssl.CERT_REQUIRED"));
    assert!(out.contains("if addressed != trust.server_name:"));
    // Both Client and AsyncClient accept it and hand it to their transport.
    assert_eq!(out.matches("trust: PrivateTrust | None = None").count(), 2);
    assert_eq!(
        out.matches("ssl_context = _private_trust_context(trust, self._base_url)")
            .count(),
        2
    );
}

#[test]
fn no_generated_client_offers_to_skip_verification() {
    let s = spec(r##"{"paths":{}}"##);
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    for weakening in [
        "CERT_NONE",
        "check_hostname = False",
        "verify=False",
        "insecure",
    ] {
        assert!(
            !out.contains(weakening),
            "generated clients must not ship {weakening}: the fix for an unverifiable \
             server is the anchor or the name, not the check"
        );
    }
}

#[test]
fn query_operation_sends_query_method_with_json_body_by_default() {
    let s = spec(
        r##"{"paths":{"/pets":{
              "query":{"operationId":"searchPets",
                "requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object"}}}},
                "responses":{"200":{"content":{"application/json":{"schema":{"type":"array","items":{"type":"string"}}}}}}}}}}"##,
    );
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    assert!(out.contains("def search_pets(self, *, body:"));
    assert!(out.contains("if self._use_post_fallback:"));
    assert!(out.contains("_method = \"POST\""));
    assert!(out.contains("_path = f\"/pets\""));
    assert!(out.contains("_method = \"QUERY\""));
    assert!(out.contains(
        "_resp = self._client.request(_method, self._base_url + _path, params=_params, headers=_headers, json=_json)"
    ));
}

#[test]
fn query_operation_honors_x_post_twin_path_override() {
    let s = spec(
        r##"{"paths":{"/pets/{petId}":{
              "query":{"operationId":"searchPetById","x-post-twin":"/pets/{petId}/search",
                "parameters":[{"name":"petId","in":"path","required":true,"schema":{"type":"integer"}}],
                "requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object"}}}},
                "responses":{"200":{"content":{"application/json":{"schema":{"type":"string"}}}}}}}}}"##,
    );
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    assert!(out.contains("_path = f\"/pets/{pet_id}/search\""));
}

#[test]
fn non_query_operation_has_no_fallback_branch() {
    let s = spec(
        r##"{"paths":{"/pets":{"get":{"operationId":"listPets","responses":{"200":{"content":{"application/json":{"schema":{"type":"array","items":{"type":"string"}}}}}}}}}}"##,
    );
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    assert!(!out.contains("if self._use_post_fallback:"));
    assert!(out.contains(
        "_resp = self._client.request(\"GET\", self._base_url + _path, params=_params, headers=_headers)"
    ));
}

#[test]
fn ndjson_operation_is_emitted_as_stream() {
    let s = spec(
        r##"{"paths":{"/logs":{"post":{"operationId":"uploadLogs",
  "requestBody":{"required":true,"content":{"application/x-ndjson":{"schema":{"type":"object"}}}},
  "responses":{"200":{"description":"ok","content":{"application/x-ndjson":{"schema":{"type":"string"}}}}}}}}}"##,
    );
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    assert!(
        out.contains(
            "    def upload_logs(self, *, body: Iterable[dict[str, Any]]) -> Iterator[str]:\n"
        ),
        "{out}"
    );
    assert!(
        out.contains("    async def upload_logs(self, *, body: AsyncIterable[dict[str, Any]] | Iterable[dict[str, Any]]) -> AsyncIterator[str]:\n"),
        "{out}"
    );
}

const NDJSON_ITEM_SCHEMA: &str = r##""components":{"schemas":{"Item":{"type":"object","properties":{"id":{"type":"integer"}},"required":["id"]}}}"##;

#[test]
fn ndjson_upload_method_streams_body_lines() {
    let s = spec(&format!(
        r##"{{"paths":{{"/items":{{"post":{{"operationId":"uploadItems",
  "requestBody":{{"required":true,"content":{{"application/x-ndjson":{{"schema":{{"$ref":"#/components/schemas/Item"}}}}}}}},
  "responses":{{"200":{{"description":"ok","content":{{"application/json":{{"schema":{{"$ref":"#/components/schemas/Item"}}}}}}}}}}}}}}}},{NDJSON_ITEM_SCHEMA}}}"##
    ));
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    assert!(
        out.contains("def upload_items(self, *, body: Iterable[Item]) -> Item:"),
        "{out}"
    );
    assert!(
        out.contains(
            "async def upload_items(self, *, body: AsyncIterable[Item] | Iterable[Item]) -> Item:"
        ),
        "{out}"
    );
    assert!(
        out.contains("_headers[\"Content-Type\"] = \"application/x-ndjson\""),
        "{out}"
    );
    assert!(
        out.contains("with self._client.stream(\"POST\", self._base_url + _path, params=_params, headers=_headers) as _stream:"),
        "{out}"
    );
    assert!(out.contains("async with self._client.stream("), "{out}");
    assert!(
        out.contains("_stream.send_data(_ndjson_line(_item))"),
        "{out}"
    );
    assert!(out.contains("_stream.end_request()"), "{out}");
    assert!(out.contains("_resp = _stream.read_response()"), "{out}");
    assert!(out.contains("import json"), "{out}");
}

#[test]
fn ndjson_response_method_is_a_generator() {
    let s = spec(&format!(
        r##"{{"paths":{{"/items":{{"get":{{"operationId":"listItems",
  "responses":{{"200":{{"description":"ok","content":{{"application/x-ndjson":{{"schema":{{"$ref":"#/components/schemas/Item"}}}}}}}}}}}}}}}},{NDJSON_ITEM_SCHEMA}}}"##
    ));
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    assert!(
        out.contains("def list_items(self) -> Iterator[Item]:"),
        "{out}"
    );
    assert!(
        out.contains("async def list_items(self) -> AsyncIterator[Item]:"),
        "{out}"
    );
    assert!(
        out.contains("yield from _ndjson_records(_stream, Item)"),
        "{out}"
    );
    assert!(
        out.contains("async for _rec in _ndjson_arecords(_stream, Item):"),
        "{out}"
    );
    assert!(
        out.contains("_headers[\"Accept\"] = \"application/x-ndjson\""),
        "{out}"
    );
}

#[test]
fn ndjson_helpers_report_malformed_lines_and_bad_status() {
    let s = spec(&format!(
        r##"{{"paths":{{"/items":{{"get":{{"operationId":"listItems",
  "responses":{{"200":{{"description":"ok","content":{{"application/x-ndjson":{{"schema":{{"$ref":"#/components/schemas/Item"}}}}}}}}}}}}}}}},{NDJSON_ITEM_SCHEMA}}}"##
    ));
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    assert!(out.contains("class NdjsonParseError(ValueError):"), "{out}");
    assert!(
        out.contains("raise NdjsonParseError(line) from None"),
        "{out}"
    );
    assert!(
        out.contains("class NdjsonStatusError(RuntimeError):"),
        "{out}"
    );
    assert!(out.contains("async def _ndjson_arecords("), "{out}");
}

#[test]
fn json_only_client_has_no_ndjson_output() {
    let s = spec(
        r##"{"paths":{"/pets":{"get":{"operationId":"listPets","responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"type":"array","items":{"type":"string"}}}}}}}}}}"##,
    );
    let tm = build_type_map(&s);
    let out = emit(&operations::build(&s), &tm, Some(PythonTarget::Py311), None);
    assert!(!out.contains("Ndjson"), "{out}");
    assert!(!out.contains("import json"), "{out}");
    assert!(!out.contains(".stream("), "{out}");
    assert!(!out.contains("AsyncIterable"), "{out}");
}

fn managed_out() -> String {
    let s = spec(
        r##"{"paths":{"/ping":{"get":{"operationId":"ping","responses":{"204":{"description":"ok"}}}}}}"##,
    );
    let tm = build_type_map(&s);
    let managed = ManagedAuth::new("/etc/ca/ca.crt", [FileBearerScheme::Https]).unwrap();
    emit_with(
        &operations::build(&s),
        &tm,
        Some(PythonTarget::Py311),
        None,
        Some(&managed),
    )
}

#[test]
fn managed_client_takes_a_ca_path_input() {
    let out = managed_out();
    assert!(out.contains("_MANAGED_CA_PATH = \"/etc/ca/ca.crt\""));
    assert_eq!(out.matches("ca_path: str | None = None").count(), 2);
    assert!(out.contains("self._ca_path = ca_path if ca_path is not None else _MANAGED_CA_PATH"));
}

#[test]
fn managed_client_refuses_non_https_before_transport() {
    let out = managed_out();
    assert!(out.contains("if urlsplit(base_url).scheme != \"https\":"));
    assert!(out.contains("managed clients accept only https: URLs"));
    assert_eq!(
        out.matches("_managed_headers(self._base_url, self._ca_path, _headers)")
            .count(),
        2
    );
    assert!(
        out.find("_managed_headers(self._base_url, self._ca_path, _headers)")
            .unwrap()
            < out.find("self._client.request(").unwrap()
    );
}

#[test]
fn managed_client_uses_the_fixed_token_path() {
    let out = managed_out();
    assert!(out
        .contains("_MANAGED_TOKEN_PATH = \"/var/run/secrets/kubernetes.io/serviceaccount/token\""));
    assert!(out.contains("_managed_read(_MANAGED_TOKEN_PATH, \"token\")"));
    assert!(!out.contains("auth_token"));
    assert!(!out.contains("token_path:"));
    assert!(!out.contains("_FILE_BEARER"));
}

#[test]
fn managed_client_fails_before_transport_on_an_empty_token_or_ca() {
    let out = managed_out();
    assert!(out.contains("managed {what} is unavailable"));
    assert!(out.contains("managed {what} is empty"));
    assert_eq!(
        out.matches("_managed_read(ca_path, \"CA bundle\")").count(),
        2
    );
}

#[test]
fn managed_client_never_retries_without_the_token() {
    let out = managed_out();
    assert_eq!(out.matches("self._client.request(").count(), 2);
    assert!(out.contains("result[\"Authorization\"] = f\"Bearer {token}\""));
    assert!(out.contains("if str(name).casefold() != \"authorization\""));
}

#[test]
fn managed_client_keeps_the_no_skip_verification_rule() {
    let out = managed_out();
    assert!(out.contains("ssl.create_default_context(cafile=ca_path)"));
    assert!(out.contains("context.check_hostname = True"));
    assert!(out.contains("context.verify_mode = ssl.CERT_REQUIRED"));
    for weakening in [
        "CERT_NONE",
        "check_hostname = False",
        "verify=False",
        "_create_unverified_context",
    ] {
        assert!(
            !out.contains(weakening),
            "managed client must not ship {weakening}"
        );
    }
}

#[test]
fn non_managed_client_is_unchanged() {
    let s = spec(
        r##"{"paths":{"/ping":{"get":{"operationId":"ping","responses":{"204":{"description":"ok"}}}}}}"##,
    );
    let tm = build_type_map(&s);
    let ops = operations::build(&s);
    let plain = emit(&ops, &tm, Some(PythonTarget::Py311), None);
    assert_eq!(
        plain,
        emit_with(&ops, &tm, Some(PythonTarget::Py311), None, None)
    );
    assert!(!plain.contains("_MANAGED_"));
    assert!(!plain.contains("_managed_headers"));
    assert!(!plain.contains("ca_path"));
}
