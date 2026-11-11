# Release notes 0.5.0: service libraries

Scope: the changes from `v0.4.14` to `origin/main` in the service libraries
listed below. Source: `git diff v0.4.14..origin/main -- crates/<crate>`, the
crate root (`lib.rs`) re-exports, and the commit subjects in
`git log v0.4.14..origin/main -- crates/<crate>`. Many crates moved their
sources into `domain/`, `application/`, `infrastructure/` and `interfaces/`
(or `api/`) folders. Those moves are not breaking by themselves; the
breaking changes below come from changed names, signatures, field
visibility and removed items.

## Crates checked

- `crates/service-auth`
- `crates/service-http`
- `crates/service-collector`
- `crates/service-observability`
- `crates/service-projection`
- `crates/service-executor`
- `crates/service-mcp`
- `crates/claim-token`

## Breaking changes

### service-auth: removed the GCE metadata ID-token source

`gcp::MetadataTokenSource`, `gcp::MetadataTokenError` and
`DEFAULT_METADATA_BASE_URL` were removed (commit `d74a448e`; the module
`infrastructure/google/metadata.rs` is gone).

Migration: an app that fetched Google ID tokens through `MetadataTokenSource`
must call the metadata server from its own code. This crate has no
replacement.

### service-auth: `async_verifier`, `role_map` and `scoped` are no longer public modules

The root modules `async_verifier`, `role_map` and `scoped` are now private
(`mod api`, `mod application`, `mod domain`, `mod infrastructure`,
`mod interfaces`). Their public items stay available at the crate root:
`AsAsync`, `AsyncVerifier`, `async_auth_middleware`, the registry items
(`Registry`, `Role`, `RoleMapPrincipal`, `StaticRoleMapVerifier`,
`load_registry*`) and the scoped items (`ScopedAuthorization`,
`scoped_authorization_middleware`).

Migration: change `service_auth::role_map::X`, `service_auth::scoped::X` and
`service_auth::async_verifier::X` to `service_auth::X`. `gcp`, `k8s`, `llm`
and `reload` stay at the root.

### service-auth: `reload::spawn_registry_files_watcher` removed

The variant without an interval was removed (commit `d74a448e`). The
interval variant stays.

Migration: call `spawn_registry_files_watcher_with_interval` and pass
`DEFAULT_REGISTRY_FILE_WATCH_INTERVAL` as the poll interval.

### service-auth: `Registry::parse` and `Registry::try_merge` return `RegistryError`

Both returned `anyhow::Result` (`Result<Self>` and `Result<()>`); they now
return `Result<Self, RegistryError>` and `Result<(), RegistryError>`
(commit `3faba63f`).

Migration: `?` still works in a function that returns `anyhow::Result`. Code
that names `anyhow::Error`, downcasts it or matches its message must match
`RegistryError` instead.

### service-auth: `TokenClaims`, `TokenReviewOutcome` and `CachePolicy` fields are private

The fields are private (commit `945641cb`). `TokenClaims` has
`TokenClaims::new(subject, roles)`, `subject() -> &str` and
`roles() -> &HashMap<String, Role>`. `TokenReviewOutcome` has the public
fields `authenticated: bool` and `error: Option<String>` removed. It is built
with `TokenReviewOutcome::authenticated(identity, audiences)` or
`TokenReviewOutcome::rejected(error)`, and read with `is_authenticated()` and
`error() -> Option<&str>`; both are in `src/domain/k8s/review.rs`.
`CachePolicy` is read through `allow_ttl()`, `deny_ttl()`, `stale_window()`,
`max_entries()` and `revocation_bound()`, all in `src/domain/k8s/cache.rs`.

Migration: replace struct literals with the constructors and field reads with
the accessors. `outcome.authenticated` becomes `outcome.is_authenticated()`,
and `outcome.error` becomes `outcome.error()`. `TokenClaims` still derives
`Deserialize`, and the wire format is unchanged.

### service-auth: `ReloadableRoleMapVerifier::reload_file` removed

`ReloadableRoleMapVerifier::reload_file(&self, path: impl AsRef<Path>) -> Result<u64>`
was removed (at `v0.4.14`, `crates/service-auth/src/reload.rs:460`). Only
`reload_files(&self, paths: &[PathBuf]) -> Result<u64>` remains, in
`src/application/role_map/reloadable.rs`.

Migration: call `reload_files(&[path.to_path_buf()])` where you called
`reload_file(path)`.

### service-auth: `TokenRequestTarget::qualified_name` removed

`TokenRequestTarget::qualified_name(&self) -> String` (at `v0.4.14`,
`crates/service-auth/src/k8s/token_request.rs:185`) no longer exists. The
accessors `namespace() -> &str` and `service_account() -> &str` remain, in
`src/application/k8s/token_request/target.rs`.

Migration: replace `target.qualified_name()` with
`format!("{}/{}", target.namespace(), target.service_account())`.

### service-http: `HttpConfig` fields are private

Seven public fields (`host`, `port`, `log_level`, `log_format`,
`body_limit_bytes`, `grace_secs`, `otlp_endpoint`) are now private (commit
`c35d7c43`). `HttpConfig::new` is unchanged.

Migration: `cfg.port` becomes `cfg.port()`, and likewise for the other six
fields. Build with `HttpConfig::new` as before.

### service-http: ingest limit fields are private

`WeightedAdmissionConfig` and `ContentDecodeLimits` fields are private
(commit `f1881059`). Their `new(...) -> Result` constructors validate the
limits and stay.

Migration: read `max_keys()`, `max_concurrent_per_key()`,
`max_weight_per_window()`, `window()` (weighted admission),
`max_compressed_bytes()` and `max_decoded_bytes()` (content decode) as
methods, not fields.

### service-http: public module paths removed

These modules are private: `admission`, `body_limit`, `config`,
`content_decode`, `error`, `logging`, `metrics`, `probes`, `readiness`,
`reverse_proxy`, `server_timing`, `signal` and `weighted_admission`. Their
public items are re-exported at the crate root, and `transport` remains
`service_http::transport`.

Migration: import from the crate root, for example
`service_http::{HttpConfig, LogFormat, serve_with_lifecycle}`. A path into a
removed module no longer compiles.

`ReadinessHook` is unchanged: at `v0.4.14` it was already
`pub use server_lifecycle::Readiness as ReadinessHook`, so an implementation
needs no change.

### service-observability: `ObservabilityConfig` fields are private

`log_level`, `log_format` and `otlp_endpoint` are private. `ObservabilityConfig::new(log_level, log_format, otlp_endpoint)`
is unchanged (commit `7ce518fe`).

Migration: read them as `log_level()`, `log_format()` and `otlp_endpoint()`.

### service-collector: `SourceProgress` fields are private and offsets are typed

`SourceProgress::start_offset` and `final_offset` were `u64` fields. They are
now methods returning `SourceOffset`, a new newtype (commit `0b799051`).
`lost_bytes()` and `lost_sources()` are methods too. The constructor is
`SourceProgress::new(start: SourceOffset, final: SourceOffset, lost_bytes, lost_sources)`.
The wire format is unchanged: `SourceOffset` serializes as the bare number.

Migration: `p.final_offset` becomes `p.final_offset().get()` where a `u64` is
needed.

### service-collector: `DeliveryReceipt` fields are private

Use `DeliveryReceipt::new(accepted, duplicates)`, `accepted()` and
`duplicates()` (commit `f6eb525f`). Struct literals no longer compile.

Migration: replace each struct literal with `DeliveryReceipt::new(accepted, duplicates)`, and read the values with `accepted()` and `duplicates()`.

### service-collector: `RuntimeConfig` is built with `try_new`; `validate` removed

`RuntimeConfig` fields are private. Build it with
`RuntimeConfig::try_new(batch_size, max_record_bytes, retry, follow, follow_poll_interval) -> Result<Self, ConfigError>`,
which rejects a zero batch size, record byte limit or follow poll interval
(commit `b5a63876`). The public `RuntimeConfig::validate(self)` no longer
exists.

Migration: replace the struct literal and each `validate()` call with
`try_new(...)`. `RetryPolicy` keeps its public fields and its `new`.

### service-projection: identities, cursors and generations are typed

`ProjectionCursor`, `ProjectionEventId`, `ProjectionName` and
`SourceGeneration` replace `u64`, `&str` and `String` (commit `244c147a`).
The wire format is unchanged: the id newtypes use `#[serde(transparent)]`,
and the checkpoint and state-envelope bytes are pinned by commit `742c957d`.

- `ProjectionRecord::projection_cursor()` returns `ProjectionCursor`;
  `projection_event_id()` returns `ProjectionEventId`.
- `ProjectionSource::current_cursor()` returns `ProjectionCursor`;
  `generation()` returns `SourceGeneration`; `read_after` takes
  `ProjectionCursor`.
- `ProjectionHandle::current_cursor()` and `catch_up()` return
  `ProjectionCursor`; `wait_for_min_cursor` takes a `ProjectionCursor` and
  returns `Result<ProjectionCursor, ProjectionLag>`.
- `ProjectionRegistry::current_cursor()` and `catch_up()` return
  `ProjectionCursor`.
- `ProjectionCheckpoint` (`src/domain/checkpoint.rs`): the public field
  `projection` is `ProjectionName` (was `String`); `cursor` and
  `source_generation` are `ProjectionCursor` and `SourceGeneration` (were
  `u64`); `event_id` is `Option<ProjectionEventId>` (was `Option<String>`).
- `ProjectionLag` (`src/domain/lag.rs`): `projection` is `ProjectionName`
  (was `String`); `required_cursor` and `current_cursor` are
  `ProjectionCursor` (were `u64`).
- `RebuildComparison` (`src/domain/rebuild.rs`): `source_cursor` and
  `rebuilt_cursor` are `ProjectionCursor` (were `u64`).

Migration: call `.get()` on a cursor where a `u64` is needed, and
`ProjectionCursor::new(n)` where you pass one in. Use `.as_str()` on a
`ProjectionName` or `ProjectionEventId` where a `&str` is needed, and build
one with `ProjectionName::new(s)` or `ProjectionEventId::new(s)` where you
pass one in.

### service-projection: `ProjectionDescriptor` is built with `try_new`

Fields are private (commit `54605286`). `ProjectionDescriptor::try_new(name: ProjectionName, schema_version: u32, retention)`
returns `ProjectionError::InvalidName` for a blank name or one that contains
`/` or NUL. Read the fields as `name()`, `schema_version()` and `retention()`.

Migration: build the descriptor with `ProjectionDescriptor::try_new(...)` and handle its `Result`. Read the fields with the accessors, not with struct literals or field access.

### service-projection: port traits return `ProjectionError`

`Projection`, `ProjectionSource` and `ProjectionReadSession` return
`Result<_, ProjectionError>` instead of `anyhow::Result` (commit `75336bd1`).
An `anyhow::Error` converts into it. Wrap any other error with
`ProjectionError::other(err)`.

Migration: change each implementation's return type to `Result<_, ProjectionError>`. Use `?` on an `anyhow::Error` (it converts), or `ProjectionError::other(err)` for any other error.

### service-projection: `ProjectionCheckpoint::empty` takes the time

`ProjectionCheckpoint::empty(descriptor, now: DateTime<Utc>)` no longer reads
the clock (commit `6886c1f5`). The old signature was `empty(descriptor)`.

Migration: pass the checkpoint time in, for example `Utc::now()`, from the
caller.

### service-executor: `GroupCommitConfig` fields are private

`max_delay`, `max_items`, `max_bytes` and `queue_capacity` are private (commit
`80c3e1f9`). Read them as `max_delay()`, `max_items()`, `max_bytes()` and
`queue_capacity()`. The constructors `new(...)` and
`with_queue_capacity(...)` are unchanged. `run_bounded` moved to a private
module and is still re-exported at the crate root.

Migration: read the fields with the accessors. Build the config with `new(...)` or `with_queue_capacity(...)` as before.

### service-mcp: None.

The crate split into `application`, `config`, `stdio` and `http` modules
(commit `ecef9152`). `McpApplication`, `HttpTransportConfig::new`,
`HttpTransportConfig::from_env`, `serve_stdio` and `streamable_http_router`
keep their names and signatures, and each is re-exported at the crate root.

### claim-token: `Scope` fields are private and keys are typed

`Scope { r, w, exp }` became private fields of type `InputKey`, `ResultKey`
and `ExpiryUnixSeconds` (commits `ab2d4e4a` and `88449eae`). Build it with
`Scope::new(InputKey, ResultKey, ExpiryUnixSeconds)` and read it with
`r()`, `w()` and `exp()`. `sign` and `verify` keep their signatures. The token
format is unchanged: the new types use `#[serde(transparent)]`, and the bytes
are pinned by commit `8e0ec813`.

Migration: `Scope { r: "in".into(), w: "out".into(), exp: 1000 }` becomes
`Scope::new(InputKey::new("in"), ResultKey::new("out"), ExpiryUnixSeconds::new(1000))`.
Read `scope.r` as `scope.r().as_str()`.

## Fixes apps must take

None. The commits that touch these crates are refactors, tests, docs, CI and
features. The one commit that changes a behavior note, `fd871f0e`
(service-observability), only corrects the documentation of the `ps`-based
process sampling: that sampling needs `ps` on `PATH`, so it is unavailable in
distroless images.

## New

- `service-auth`: `ProjectedTokenMode` (projected-token mode) and
  `ProjectedToken` in the domain layer, for the portable opaque
  projected-token source (`15c26d4c`); `RegistryError`, the typed registry
  error (`3faba63f`).
- `service-http`: accessors on `HttpConfig`, `WeightedAdmissionConfig` and
  `ContentDecodeLimits` (listed under Breaking changes).
- `service-collector`: `SourceOffset`, the typed byte offset (`0b799051`).
- `service-projection`: `ProjectionName`, `ProjectionCursor`,
  `ProjectionEventId`, `SourceGeneration` and `ProjectionError` (`244c147a`,
  `75336bd1`).
- `claim-token`: `InputKey`, `ResultKey` and `ExpiryUnixSeconds` (`ab2d4e4a`).
