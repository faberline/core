use super::*;
use crate::emit::ts::plan;
use crate::ir::openapi::Spec;
use crate::{build_type_map, FileBearerScheme, GenOptions};
use crate::{FileBearerAuth, HttpClient};
use std::path::PathBuf;

fn opts() -> GenOptions {
    GenOptions::new(
        crate::Lang::Ts,
        PathBuf::new(),
        PathBuf::new(),
        "createClient",
    )
}

fn render(json: &str) -> String {
    let s: Spec = serde_json::from_str(json).unwrap();
    let tm = build_type_map(&s);
    let plans = plan::build(&s, &tm);
    emit_client(&plans, &opts())
}

#[test]
fn client_method_takes_grouped_data() {
    let out = render(
        r##"{"components":{"schemas":{"Pet":{"type":"object","properties":{"id":{"type":"integer"}}}}},
            "paths":{"/pets/{petId}":{"get":{"operationId":"getPetById",
              "parameters":[{"name":"petId","in":"path","required":true,"schema":{"type":"integer"}}],
              "responses":{"200":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Pet"}}}}}}}}}"##,
    );
    assert!(out.contains("import type { GetPetByIdData, GetPetByIdResponse } from \"./types\";"));
    assert!(out.contains("getPetById(data: GetPetByIdData): Promise<GetPetByIdResponse> {"));
    assert!(out.contains("return request<GetPetByIdResponse>(config, { method: \"GET\", path: `/pets/${data.path.petId}` });"));
}

#[test]
fn client_query_and_body_access() {
    let out = render(
        r##"{"paths":{"/pets":{
              "get":{"operationId":"listPets","parameters":[{"name":"limit","in":"query","required":false,"schema":{"type":"integer"}}],
                "responses":{"200":{"content":{"application/json":{"schema":{"type":"array","items":{"type":"string"}}}}}}},
              "post":{"operationId":"createPet","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object"}}}},
                "responses":{"201":{"content":{"application/json":{"schema":{"type":"string"}}}}}}}}}"##,
    );
    assert!(out.contains("listPets(data: ListPetsData): Promise<ListPetsResponse> {"));
    assert!(out.contains("query: { limit: data.query?.limit }"));
    assert!(out.contains("createPet(data: CreatePetData): Promise<CreatePetResponse> {"));
    assert!(out.contains("body: data.body"));
}

#[test]
fn no_input_operation_takes_no_arg() {
    let out = render(
        r##"{"paths":{"/health":{"get":{"operationId":"health","responses":{"200":{"content":{"application/json":{"schema":{"type":"boolean"}}}}}}}}}"##,
    );
    assert!(out.contains("health(): Promise<HealthResponse> {"));
    assert!(out.contains("import type { HealthResponse } from \"./types\";"));
}

#[test]
fn void_operation_marks_runtime_to_skip_body_parse() {
    let out = render(
        r##"{"paths":{"/health":{"get":{"operationId":"health","responses":{"200":{"description":"ok"}}}}}}"##,
    );
    assert!(out.contains("health(): Promise<HealthResponse> {"));
    assert!(out.contains(
        "return request<HealthResponse>(config, { method: \"GET\", path: `/health`, expectBody: false });"
    ));
}

#[test]
fn query_operation_emits_query_method_with_post_fallback_ternary() {
    let out = render(
        r##"{"paths":{"/pets":{
              "query":{"operationId":"searchPets",
                "requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object"}}}},
                "responses":{"200":{"content":{"application/json":{"schema":{"type":"array","items":{"type":"string"}}}}}}}}}}"##,
    );
    assert!(out.contains("searchPets(data: SearchPetsData): Promise<SearchPetsResponse> {"));
    assert!(out.contains("method: config.usePostFallback ? \"POST\" : \"QUERY\""));
    assert!(out.contains("path: config.usePostFallback ? `/pets` : `/pets`"));
    assert!(out.contains("body: data.body"));
}

#[test]
fn query_operation_honors_x_post_twin_path_in_fallback_branch() {
    let out = render(
        r##"{"paths":{"/pets/{petId}":{
              "query":{"operationId":"searchPetById","x-post-twin":"/pets/{petId}/search",
                "parameters":[{"name":"petId","in":"path","required":true,"schema":{"type":"integer"}}],
                "requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object"}}}},
                "responses":{"200":{"content":{"application/json":{"schema":{"type":"string"}}}}}}}}}"##,
    );
    assert!(out.contains(
        "path: config.usePostFallback ? `/pets/${data.path.petId}/search` : `/pets/${data.path.petId}`"
    ));
}

#[test]
fn client_config_declares_use_post_fallback_flag() {
    let fetch = emit_runtime(HttpClient::Fetch, None);
    assert!(fetch.contains("usePostFallback?: boolean;"));
    let axios = emit_runtime(HttpClient::Axios, None);
    assert!(axios.contains("usePostFallback?: boolean;"));
}

#[test]
fn both_runtimes_take_a_private_ca_and_verify_the_name_they_address() {
    for runtime in [
        emit_runtime(HttpClient::Fetch, None),
        emit_runtime(HttpClient::Axios, None),
    ] {
        assert!(runtime.contains("export interface PrivateTrust {"));
        assert!(runtime.contains("trust?: PrivateTrust;"));
        assert!(runtime.contains("if (addressed !== trust.serverName) {"));
        // A trust anchor nothing consults is worse than none: the caller
        // believes they pinned it. Both runtimes refuse that combination.
        assert!(runtime.contains("assertTrustIsHonoured(config,"));
    }
    assert!(emit_runtime(HttpClient::Fetch, None)
        .contains("export async function privateCaFetch(trust: PrivateTrust)"));
    assert!(emit_runtime(HttpClient::Axios, None)
        .contains("export async function privateCaAxios(trust: PrivateTrust)"));
}

#[test]
fn no_generated_runtime_offers_to_skip_verification() {
    for runtime in [
        emit_runtime(HttpClient::Fetch, None),
        emit_runtime(HttpClient::Axios, None),
    ] {
        for weakening in [
            "rejectUnauthorized: false",
            "NODE_TLS_REJECT_UNAUTHORIZED",
            "insecure",
        ] {
            assert!(
                !runtime.contains(weakening),
                "generated clients must not ship {weakening}: the fix for an unverifiable \
                 server is the anchor or the name, not the check"
            );
        }
    }
}

#[test]
fn both_node_runtimes_resolve_file_bearer_before_admission() {
    let auth = FileBearerAuth::new(
        "/private/tmp/token",
        ".example.internal",
        [FileBearerScheme::Http, FileBearerScheme::Https],
    )
    .unwrap();
    for runtime in [
        emit_runtime(HttpClient::Fetch, Some(&auth)),
        emit_runtime(HttpClient::Axios, Some(&auth)),
    ] {
        assert!(runtime.contains("node:fs/promises"));
        assert!(runtime.contains("hasAuthorization(headers)"));
        assert!(runtime.contains("const headers = await attachFileBearer"));
        assert!(runtime.contains("      headers,"));
        assert!(
            runtime.find("await attachFileBearer").unwrap()
                < runtime.find("await acquireAdmission").unwrap()
        );
    }
}

#[test]
fn runtime_fetch_and_axios() {
    let fetch = emit_runtime(HttpClient::Fetch, None);
    assert!(fetch.contains("export interface TransportPolicy"));
    assert!(fetch.contains("const DEFAULT_MAX_CONNECTIONS = 128;"));
    assert!(fetch.contains("const DEFAULT_MAX_KEEPALIVE_CONNECTIONS = 16;"));
    assert!(fetch.contains("export function recommendedH2Connections"));
    assert!(fetch.contains("async function acquireAdmission"));
    assert!(fetch.contains("const doFetch = config.fetch ?? fetch;"));
    assert!(fetch.contains("const release = await acquireAdmission(config, url);"));
    assert!(fetch.contains("args.expectBody === false || response.status === 204"));
    assert!(!fetch.contains("axios"));
    let axios = emit_runtime(HttpClient::Axios, None);
    assert!(axios.contains("export interface TransportPolicy"));
    assert!(axios.contains("const DEFAULT_MAX_CONNECTIONS = 128;"));
    assert!(axios.contains("const DEFAULT_MAX_KEEPALIVE_CONNECTIONS = 16;"));
    assert!(axios.contains("async function acquireAdmission"));
    assert!(axios.contains("import axios from \"axios\";"));
    assert!(axios.contains("config.axios ?? axios.create()"));
    assert!(axios.contains("const release = await acquireAdmission(config, url);"));
    assert!(axios.contains("if (args.expectBody === false)"));
    assert!(axios.contains("return response.data;"));
}
