use super::*;
use crate::emit::ts::plan;
use crate::ir::openapi::Spec;
use crate::ManagedAuth;
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

#[test]
fn ndjson_upload_method_takes_async_iterable() {
    let out = render(
        r##"{"paths":{"/items":{"post":{"operationId":"uploadItems",
          "requestBody":{"required":true,"content":{"application/x-ndjson":{"schema":{"type":"object","properties":{"id":{"type":"integer"}}}}}},
          "responses":{"200":{"content":{"application/json":{"schema":{"type":"string"}}}}}}}}}"##,
    );
    assert!(out.contains(
        "uploadItems(data: UploadItemsData, options?: NdjsonOptions): Promise<UploadItemsResponse> {"
    ));
    assert!(out.contains(
        "return requestNdjsonUpload<UploadItemsResponse>(config, { method: \"POST\", path: `/items`, ndjsonBody: data.body }, options);"
    ));
    assert!(out.contains("import { requestNdjsonUpload } from \"./runtime\";"));
    assert!(!out.contains(", body: data.body"));
}

#[test]
fn ndjson_response_method_returns_async_iterable() {
    let out = render(
        r##"{"paths":{"/items":{"get":{"operationId":"listItems",
          "responses":{"200":{"content":{"application/x-ndjson":{"schema":{"type":"object","properties":{"id":{"type":"integer"}}}}}}}}}}}"##,
    );
    assert!(out.contains("listItems(options?: NdjsonOptions): ListItemsResponse {"));
    assert!(out.contains(
        "return requestNdjsonStream<NdjsonItem<ListItemsResponse>>(config, { method: \"GET\", path: `/items` }, options);"
    ));
    assert!(out.contains("import { requestNdjsonStream } from \"./runtime\";"));
    assert!(out.contains("import type { NdjsonItem, NdjsonOptions } from \"./runtime\";"));
}

#[test]
fn json_only_client_has_no_ndjson_imports() {
    let out = render(
        r##"{"paths":{"/pets":{
          "get":{"operationId":"listPets","parameters":[{"name":"limit","in":"query","required":false,"schema":{"type":"integer"}}],"responses":{"200":{"content":{"application/json":{"schema":{"type":"array","items":{"type":"string"}}}}}}},
          "post":{"operationId":"createPet","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object"}}}},"responses":{"201":{"content":{"application/json":{"schema":{"type":"string"}}}}}}}}}"##,
    );
    assert!(!out.contains("Ndjson"));
    assert!(out.contains("import { request } from \"./runtime\";"));
}

#[test]
fn fetch_runtime_has_ndjson_helpers() {
    let fetch = emit_runtime(HttpClient::Fetch, None);
    assert!(fetch.contains("export async function* requestNdjsonStream<T>("));
    assert!(fetch.contains("export async function requestNdjsonUpload<T>("));
    assert!(fetch.contains("export class NdjsonParseError extends Error"));
    assert!(fetch.contains("signal: options.signal"));
    assert!(fetch.contains("init.duplex = \"half\""));
}

#[test]
fn axios_runtime_has_ndjson_helpers() {
    let axios = emit_runtime(HttpClient::Axios, None);
    assert!(axios.contains("export async function* requestNdjsonStream<T>("));
    assert!(axios.contains("export async function requestNdjsonUpload<T>("));
    assert!(axios.contains("export class NdjsonParseError extends Error"));
    assert!(axios.contains("responseType"));
}

#[test]
fn file_bearer_runtime_covers_ndjson_helpers() {
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
        assert!(runtime.contains(
            "await attachFileBearer(config.baseUrl, { \"Content-Type\": args.ndjsonBody ? \"application/x-ndjson\""
        ));
    }
}

fn managed() -> ManagedAuth {
    ManagedAuth::new("/etc/ca/ca.crt", [FileBearerScheme::Https]).unwrap()
}

fn managed_runtimes() -> [(HttpClient, String); 2] {
    [
        (
            HttpClient::Fetch,
            emit_managed_runtime(HttpClient::Fetch, &managed()),
        ),
        (
            HttpClient::Axios,
            emit_managed_runtime(HttpClient::Axios, &managed()),
        ),
    ]
}

#[test]
fn managed_runtime_takes_a_ca_path_input() {
    for (client, runtime) in managed_runtimes() {
        assert!(runtime.contains("caPath?: string;"), "{client:?}");
        assert!(
            runtime.contains("const MANAGED_CA_PATH = \"/etc/ca/ca.crt\";"),
            "{client:?}"
        );
        assert!(
            runtime.contains("config.caPath ?? MANAGED_CA_PATH"),
            "{client:?}"
        );
    }
}

#[test]
fn managed_runtime_refuses_non_https_before_transport() {
    for (client, runtime) in managed_runtimes() {
        assert!(
            runtime.contains("if (url.protocol !== \"https:\") {"),
            "{client:?}"
        );
        assert!(
            runtime.contains("managed clients accept only https: URLs"),
            "{client:?}"
        );
        let prepare = runtime.find("await prepareManaged(config, url)").unwrap();
        let admission = runtime.find("await acquireAdmission").unwrap();
        let transport = match client {
            HttpClient::Fetch => runtime.find("doFetch(url.toString()").unwrap(),
            HttpClient::Axios => runtime.find("instance.request<T>(").unwrap(),
        };
        assert!(prepare < admission, "{client:?}");
        assert!(admission < transport, "{client:?}");
    }
}

#[test]
fn managed_runtime_uses_the_fixed_token_path() {
    for (client, runtime) in managed_runtimes() {
        assert!(
            runtime.contains(
                "const MANAGED_TOKEN_PATH = \"/var/run/secrets/kubernetes.io/serviceaccount/token\";"
            ),
            "{client:?}"
        );
        assert!(!runtime.contains("tokenPath"), "{client:?}");
        assert!(!runtime.contains("token?: string"), "{client:?}");
        assert!(
            runtime.contains("readManagedFile(MANAGED_TOKEN_PATH, \"token\")"),
            "{client:?}"
        );
    }
}

#[test]
fn managed_runtime_fails_before_transport_on_an_empty_token_or_ca() {
    for (client, runtime) in managed_runtimes() {
        assert!(
            runtime.contains("managed ${what} is unavailable"),
            "{client:?}"
        );
        assert!(runtime.contains("managed ${what} is empty"), "{client:?}");
        assert!(
            runtime.contains("readManagedFile(config.caPath ?? MANAGED_CA_PATH, \"CA bundle\")"),
            "{client:?}"
        );
        let ca_read = runtime.find("\"CA bundle\"").unwrap();
        let admission = runtime.find("await acquireAdmission").unwrap();
        assert!(ca_read < admission, "{client:?}");
    }
}

#[test]
fn managed_runtime_never_retries_without_the_token() {
    let fetch = emit_managed_runtime(HttpClient::Fetch, &managed());
    assert_eq!(fetch.matches("doFetch(url.toString()").count(), 2);
    assert_eq!(
        fetch.matches("await prepareManaged(config, url)").count(),
        2
    );
    let axios = emit_managed_runtime(HttpClient::Axios, &managed());
    assert_eq!(axios.matches("instance.request").count(), 2);
    assert_eq!(
        axios.matches("await prepareManaged(config, url)").count(),
        2
    );
    for runtime in [&fetch, &axios] {
        assert_eq!(
            runtime
                .matches("...args.headers, Authorization: managed.authorization")
                .count(),
            2
        );
        assert!(!runtime.contains("attachFileBearer"));
        assert!(!runtime.contains("hasAuthorization"));
    }
}

#[test]
fn managed_runtime_covers_ndjson_requests() {
    for (client, runtime) in managed_runtimes() {
        let start = runtime.find("async function sendNdjson(").unwrap();
        let tail = &runtime[start..];
        let prepare = tail.find("await prepareManaged(config, url)").unwrap();
        let admission = tail.find("await acquireAdmission").unwrap();
        assert!(prepare < admission, "{client:?}");
        assert!(
            runtime.contains(
                "Accept: accept, ...config.headers, ...args.headers, Authorization: managed.authorization };"
            ),
            "{client:?}"
        );
    }
}

#[test]
fn managed_runtime_replacements_applied() {
    let fetch = emit_managed_runtime(HttpClient::Fetch, &managed());
    assert!(!fetch.contains("const doFetch = config.fetch ?? fetch;"));
    assert!(fetch.contains("managedFetch(managed.ca, url.hostname)"));
    let axios = emit_managed_runtime(HttpClient::Axios, &managed());
    assert!(!axios.contains("config.axios ?? axios.create()"));
    assert!(axios.contains("managedAxios(managed.ca, url.hostname)"));
}

#[test]
fn managed_runtime_keeps_the_no_skip_verification_rule() {
    for (client, runtime) in managed_runtimes() {
        assert!(!runtime.contains("rejectUnauthorized: false"), "{client:?}");
        assert!(
            !runtime.contains("NODE_TLS_REJECT_UNAUTHORIZED"),
            "{client:?}"
        );
        assert!(!runtime.contains("insecure"), "{client:?}");
    }
}

#[test]
fn non_managed_runtime_is_unchanged() {
    for client in [HttpClient::Fetch, HttpClient::Axios] {
        let runtime = emit_runtime(client, None);
        assert!(!runtime.contains("MANAGED_"), "{client:?}");
        assert!(!runtime.contains("prepareManaged"), "{client:?}");
        assert!(!runtime.contains("caPath"), "{client:?}");
    }
}
