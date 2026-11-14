# openapi-codegen status

## Scope

This document describes the current source contract for the reusable
`openapi-codegen` crate. It does not claim that the named gate ran in this
working session.

Use the [README](README.md) for the generation workflow. Use the
[roadmap](ROADMAP.md) for future outcomes and explicit non-goals.

## State definitions

| State | Meaning |
|---|---|
| Supported | The current source has a public contract, an implementation, and a named executable gate for the stated scope. |
| Limited | The current source supports the stated scope, but the Limits cell names a material boundary. |
| Not supported | The behavior is not part of the current product contract. The Evidence cell points to a future outcome or a non-goal. |

## Support matrix

| Surface | ID | State | Supported scope | Limits | Evidence |
|---|---|---|---|---|---|
| TypeScript, Python, and Rust generation | `three-language-generation` | Supported | One OpenAPI document can produce typed models and operation methods for all three languages. | The generator supports its modeled OpenAPI subset, not every possible extension. | `cargo test -p openapi-codegen` |
| Versioned target profiles | `versioned-target-profiles` | Supported | Explicit Python, TypeScript, and Rust profiles control compatible syntax and emit deterministic target metadata. | No target keeps the legacy output and emits no manifest. | `cargo test -p openapi-codegen` |
| QUERY and POST fallback | `query-post-fallback` | Supported | OpenAPI 3.2 `query` operations emit an HTTP `QUERY` method and a runtime POST-twin choice in all three languages. | The service contract must provide a valid sibling or `x-post-twin` path. | `cargo test -p openapi-codegen` |
| Arbitrary additional operations | `arbitrary-additional-operations` | Limited | `additionalOperations` entries parse into the shared operation model without failing generation. | Emitters do not create typed methods for arbitrary methods beyond the dedicated `QUERY` path. See [Additional operation emission](ROADMAP.md#additional-operation-emission). | `cargo test -p openapi-codegen` |
| JSON operation model | `json-operation-model` | Limited | The shared operation model reads `application/json` request bodies and JSON responses for generated methods. | Non-JSON media types other than NDJSON are not represented. See [Media type and streaming operations](ROADMAP.md#media-type-and-streaming-operations). | `cargo test -p openapi-codegen` |
| Streaming operation emission | `streaming-operation-emission` | Limited | NDJSON (`application/x-ndjson`) request bodies and response bodies become streaming methods in all three languages: TypeScript takes and returns async iterables (`requestNdjsonUpload`, `requestNdjsonStream`, abort through `NdjsonOptions.signal`), Python takes an iterable and returns a generator (an async iterable and async generator on the async client), and Rust takes an iterator and returns `NdjsonRecords<T>` (dropping it cancels the request). A response is read one line at a time, so the whole stream is not buffered. A malformed line raises `NdjsonParseError` (TypeScript, Python) or yields `ClientError::Ndjson { line, reason }` (Rust). A normal JSON operation in the same API is unchanged. | Only NDJSON is streamed. There is no cap on the size of one line: a single very long line is buffered whole. Rust streaming uses the blocking client only. The generated TypeScript and Python streaming code is checked by output-string tests and a compile check, not run against a server. Only the Rust client is run against a stub server (`generated_rust_client_streams_ndjson_records`). Other streaming media types are not supported. See [Media type and streaming operations](ROADMAP.md#media-type-and-streaming-operations). | `cargo test -p openapi-codegen` (`ndjson_upload_method_takes_async_iterable`, `ndjson_upload_method_streams_body_lines`, `generated_rust_client_streams_ndjson_records`) |
| Structured client errors | `structured-client-errors` | Limited | Rust decodes declared non-success responses into a per-operation typed error. | TypeScript and Python still surface transport-level failures only. | [Structured client errors](ROADMAP.md#structured-client-errors) |
| Rust server skeleton | `rust-server-skeleton` | Limited | With `GenOptions::with_emit_server(true)` the Rust emitter writes `server.rs`: a handler trait with one async method per operation, one error enum per operation with an `IntoResponse` that returns the declared status and a `{error, message}` JSON body, and `router(handler, readiness, openapi_json)` that serves every operation and the `service-http` standard probe routes. | Success always answers 200 or 204, not the declared 2xx code. Header parameters, NDJSON operations, `QUERY` and other non-standard methods have no route (a generator warning names them). The `openapi_json` function and the readiness hook come from the caller. TypeScript and Python have no server mode. | `cargo test -p openapi-codegen` (`router_has_one_route_per_operation`, `generated_router_serves_every_route`, `generated_error_types_render_declared_status_and_json_body`) |
| Rust schema type parity | `rust-schema-type-parity` | Limited | Rust emits ordinary object fields and scalar types for the supported schema subset. | Component `oneOf`/`anyOf` unions tagged by a `discriminator` or distinguishable (distinct struct `$ref`s or distinct primitives) are serde enums; indistinguishable and inline unions stay `serde_json::Value`; inline string enums stay `String`. See [Cross-language type parity](ROADMAP.md#cross-language-type-parity). | `cargo test -p openapi-codegen` |
| Private service trust | `private-service-trust` | Supported | Generated clients can replace public roots with a supplied private CA and require the base URL to match the asserted server name. | The caller must distribute the public CA material. No skip-verification mode exists. | `cargo test -p openapi-codegen` |
| Static request auth values | `static-request-auth-values` | Limited | TypeScript accepts fixed default headers. Python accepts a fixed token or fixed default headers at construction. | Rust has no equivalent static constructor input. The opt-in file-bearer extension is the separate path. See [Dynamic request auth provider](ROADMAP.md#dynamic-request-auth-provider). | `cargo test -p openapi-codegen --locked` |
| Target dependency manifest | `target-dependency-manifest` | Supported | Explicit target profiles emit complete provenance (OpenAPI SHA, generator version), caller-supplied compatibility, and runtime dependencies next to generated source. | Package publication and package registry selection remain non-goals. | `cargo test -p openapi-codegen` |
| Dynamic per-request auth provider | `dynamic-per-request-auth-provider` | Limited | An explicit generation-time generic file-bearer extension rereads a configured file once before each eligible request and fails before transport on provider errors. | Legacy generation remains default-off. A general injected provider, cancellation contract, and app-owned identity policy remain future work. See [Dynamic request auth provider](ROADMAP.md#dynamic-request-auth-provider). | `cargo test -p openapi-codegen --locked` |
| Operation-aware retry hooks | `operation-aware-retry-hooks` | Not supported | Generated clients do not expose a shared hook for an app-supplied retry policy, deadline, cancellation, backoff, or `Retry-After`. | The generator must not guess which service writes are idempotent. | [Operation-aware retry hooks](ROADMAP.md#operation-aware-retry-hooks) |
| Strict cross-language execution gate | `strict-cross-language-execution-gate` | Not supported | No required harness proves generated TypeScript, Python, and Rust output in one clean run. | A consumer gate can still skip a language when its toolchain is absent. | [Strict cross-language generation gates](ROADMAP.md#strict-cross-language-generation-gates) |
| Credential acquisition from OpenAPI | `credential-acquisition-from-openapi` | Not supported | A bearer security scheme does not make the generator obtain a token or select an identity system. | Apps and auth libraries own credential sources and service policy. | [Identity policy and token acquisition](ROADMAP.md#identity-policy-and-token-acquisition) |
| Package publication | `package-publication` | Not supported | The library emits source and target metadata into a caller-selected directory. | It does not publish or maintain generated npm, PyPI, or crates.io packages. | [Package publication](ROADMAP.md#package-publication) |

## Evidence policy

The commands above are required gates for each supported scope. This document
does not store execution output. CI and local test logs own run evidence.

Update a row with any public API, behavior, or evidence change. Move a future
outcome into current support only after its implementation and executable gate
exist.
