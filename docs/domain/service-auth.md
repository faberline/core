# service-auth

service-auth answers "who is this caller?" for a faberline HTTP service. It
extracts the bearer token, verifies it with a verifier the service chooses (a
static or reloadable token-to-role registry, Google identity tokens, or
Kubernetes TokenReview and SubjectAccessReview), rejects failures without
exposing the credential, and hands a principal to service code. What a
resource means and which role it needs stay with the service. In core,
service-mcp and service-backup use it; downstream, beam, courier, defer, keep,
lumen, relay, sift and tape do.

**Form:** layered · **Depends on:** cli-std, metrics-prometheus · **Crate:** [`crates/service-auth`](../../crates/service-auth)

## Model

- **Verifier** — `Verifier`, `AsyncVerifier` and `ScopedAuthorization`: turn
  request headers into a service-defined principal or an `AuthError`. Open
  mode is a verifier choice (`required()`), not a middleware switch.
- **Auth role** — `Role`: `Read`, `Write`, `Admin`; a higher role covers a
  lower one.
- **Token claims** — `TokenClaims`: a subject and its role per resource, where
  `*` matches any resource. Built with `TokenClaims::new(subject, roles)` (or
  deserialized) and read through `subject()` and `roles()`.
- **Registry** — `Registry`: bearer tokens and identities in two disjoint
  namespaces, each mapped to `TokenClaims`, loaded from one or more
  `RegistrySource` files.
- **Role-map principal** — `RoleMapPrincipal` (`Open` or a token's claims);
  `ensure(resource, needed)` yields `RoleMapDenied` when the role falls short.
- **Reloadable verifier** — `ReloadableRoleMapVerifier`: a registry snapshot
  with a revision that increases on each successful reload.
- **Audit event** — `AuthEvent`: a registry reload or an
  `AuthorizationDecision` with its `AuthorizationReason`.
- **Google credential** — `Credential`: a Google ID token or an opaque access
  token, verified by `GoogleVerifier` under `GoogleAuthConfig`.
- **Delegated authentication** — `DelegatedAuthenticator`: TokenReview and
  SubjectAccessReview behind a `TtlCache` governed by a `CachePolicy`
  (`Default` plus `with_*` builders), admitting only a
  `ServiceAccountPrincipal`. A review answer is a `TokenReviewOutcome`, built
  with `authenticated`, `rejected` or `new`.
- **Token fingerprint** — `fingerprint`: the first 6 bytes of a token's sha256,
  for correlating audit lines.
- **Workload tokens** — `ProjectedTokenFile` (a mounted ServiceAccount token),
  and `TokenSource`, a struct that holds a `MintedToken` from a `TokenMinter`
  and refreshes it before expiry.

## Ports

- `Verifier`, `AsyncVerifier`, `ScopedAuthorization` — implemented by services:
  keep, lumen and sift respectively.
- `AuthEventSink` — audit output; `NoopAuthEventSink`, `TracingAuthEventSink`.
- `AccessTokenIntrospection` (domain) — Google access-token introspection;
  `HttpAccessTokenIntrospection`.
- `ReviewBackend` (domain, with its outcome, attribute and error types) —
  TokenReview and SubjectAccessReview; `KubeReviewBackend` (feature `k8s`),
  and fakes in lumen and sift.
- The two domain ports are written by hand in the form `#[async_trait]`
  expands to, so they stay dyn-compatible and an `#[async_trait]` impl
  compiles unchanged.
- `JwksSource` (application, `#[async_trait]`) — Google key sets;
  `HttpJwksSource`. It stays in application because it returns
  `jsonwebtoken::jwk::JwkSet`, which the domain may not name.
- `TokenMinter` (application, `#[async_trait]`) — the TokenRequest API;
  `KubeTokenMinter` (feature `k8s`). It stays in application because the
  interfaces `LoopbackProxy::next_fatal` names its `TokenRequestError`.
- Two clocks: the seconds clock `gcp::Clock` and the millisecond clock
  `k8s::Clock` (with `ManualClock` for tests), each with a `SystemClock`
  adapter in infrastructure. The constructors that default to a
  `SystemClock` or wire the HTTPS sources (`DelegatedAuthenticator::new`,
  `TokenSource::new`, `GoogleVerifier::google`) live in the `src/app`
  composition root.

## Invariants

- `AuthError` renders 401 `unauthenticated`, 403, or 503 `auth_unavailable`
  with an `{error, message}` body. Google upstream failures are 503; other
  Google failures are 401.
- A registry is invalid when required but empty, when a key, subject or
  resource is empty, or when an identity key lacks `@`. Merging registries
  rejects a key that appears twice in one namespace. `Registry::parse` and
  `Registry::try_merge` return a `RegistryError` that never names a bearer
  secret; the loaders pass it on as `anyhow::Error`. `load_registry` accepts
  bearer tokens only.
- A reload parses and validates before it takes the write lock, so a failure
  keeps the last known good registry; `reload_files` is all-or-nothing.
- Audit events carry no credential.
- Delegated authentication never produces an allow that the apiserver did not:
  a positive answer comes only from a live review or a cache entry a live
  review wrote, and an explicit deny wins. The cache is keyed by the token's
  SHA-256; a stale entry is reused only after a failed review and only within
  `stale_window`. `DelegatedAuthConfig::new` rejects an empty audience list.
- `ServiceAccountPrincipal` parses only `system:serviceaccount:<ns>:<name>`
  with DNS-1123 labels; a `PrincipalRejection` never echoes the value.
- `ProjectedTokenFile` rereads the file on every call and checks shape, expiry
  and audience, never the signature. `ProjectedToken` redacts itself in
  `Debug` and `Display`.

## Published language

Most consumers import from the crate root: the middleware, `bearer_token`,
`AuthError`, the verifier traits, the role map and registry loaders, the
reloadable verifier with its audit types, and the Google verifier and its
ports. The Kubernetes types are published under `k8s::` (and lumen asserts
that path in its own sources). Four public modules keep their paths because
they hold names the root does not re-export (`src/api/`): `gcp`, `k8s` (with
`cache`, `delegated`, `loopback_proxy`, `principal`, `projected`, `review`,
`token_request` and `kube_backend`), `llm` and `reload`. P2 deleted the old
modules `async_verifier`, `role_map` and `scoped`: every name in them is at
the crate root, so `role_map::Role`, which lumen used, is now `Role`.
Downstream uses `llm::topic` and `k8s::*` by path. `llm` is the only use of
cli-std: an llm topic.

## Exceptions and debts

- **Checker exceptions:**
  - B3 `infrastructure->application`, narrowed by P2 to two files:
    `google/http.rs` (`HttpJwksSource` implements the application
    `JwksSource`) and `k8s/kube_token_minter.rs` (`KubeTokenMinter`
    implements the application `TokenMinter`). See Ports for why those two
    ports stay in application. P2 moved `ReviewBackend` and
    `AccessTokenIntrospection` to the domain, which cleared
    `k8s/kube_backend.rs`.
  - Other contexts: `bearer_token` is in the application layer, so service-mcp
    uses it without an exception. service-backup reads `k8s::ProjectedTokenFile`
    only from its own `src/app` composition root, so it needs no exception.
- **Tracked:** `anyhow` in the registry loaders and the reload API (ADR D4).
  `ReviewedIdentity` has public fields built with struct literals in lumen
  and sift. `DelegatedAuthMetrics` exposes its counters as public fields. The
  two clock ports stay separate.
