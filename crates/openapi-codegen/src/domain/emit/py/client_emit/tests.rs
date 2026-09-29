use super::*;
use crate::ir::openapi::Spec;
use crate::ir::operations;
use crate::{build_type_map, PythonTarget};

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
