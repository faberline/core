# Release notes 0.5.0: server, transport and metrics crates

Scope: the changes from `v0.4.14` to `origin/main` in the crates listed below.
Source: `git diff v0.4.14..origin/main -- crates/<crate>`, the crate root
(`lib.rs`) re-exports, and the commit subjects in
`git log v0.4.14..origin/main -- crates/<crate>`. Many crates moved their
sources into separate modules (for example `options`, `serve`, `trace`,
`config`, `connection`, `report`, `handler`, `codec`, `consumer`, `payload`).
Those moves are not breaking by themselves. Each root re-export named below
still resolves, unless the entry says otherwise.

No crate in this list changed its `[features]` table. The `Cargo.toml`
changes are the removal of `e2e/` test declarations, the switch to workspace
lints, and dependency changes that do not change the public API.

## Crates checked

- `crates/server-http`
- `crates/server-lifecycle`
- `crates/server-tcp`
- `crates/transport-h2c`
- `crates/transport-otlp`
- `crates/peer-tls`
- `crates/metrics-prometheus`
- `crates/metrics-remote-write`

## Breaking changes

### server-http: `HttpServerOptions` and `TlsServerOptions` fields are private

The public fields of `HttpServerOptions` (`max_concurrent_streams`,
`drain_timeout`, `connection_budget`, `drain`, `socket`,
`connection_metrics`) and of `TlsServerOptions` (`http`, `metrics`) are
private (commit `67e6fbc1`).

Migration: replace a struct literal or a field write with the `with_*`
builders, for example `with_drain_timeout(d)`, `with_socket(s)` and
`with_http(h)`. Read values with the getters, for example `drain_timeout()`,
`connection_budget() -> Option<&ConnectionBudget>`, `drain() -> &DrainController`
and `socket() -> TcpSocketOptions`. `set_drain_timeout(&mut self, d)` changes a
value in place.

### server-http: `HttpServerReport` is a type alias

`HttpServerReport` was a struct. It is now `pub type HttpServerReport = server_tcp::TcpServerReport`
(commit `faa4717c`). The name still resolves.

Migration: code that builds an `HttpServerReport` with a struct literal, or
matches its fields, must use the fields of `server_tcp::TcpServerReport`.
Compare the field list before you upgrade.

### server-tcp: `TcpHandler` futures resolve to `TcpHandlerError`

`TcpHandler::Future` was `Future<Output = anyhow::Result<()>>`. It is now
`Future<Output = Result<(), TcpHandlerError>>` (commit `15acefca`). The new
type is re-exported from the crate root with `TcpHandler`.

Migration: an implementation that returns an error must convert it with
`TcpHandlerError::other(err)`. The method accepts any error that converts into
`Box<dyn Error + Send + Sync>`, including an `anyhow::Error`. The log text
stays the same.

### server-tcp: config, socket and connection-result fields are private

- `TcpServerConfig`: `bind`, `connection_budget`, `drain`, `socket`,
  `drain_timeout` and `connection_metrics` are private. Read them through
  `bind()`, `connection_budget()`, `drain()`, `socket()`, `drain_timeout()`
  and `connection_metrics()`.
- `TcpSocketOptions`: `backlog`, `reuse_addr` and `nodelay` are private. Set
  them with `with_backlog`, `with_reuse_addr` and `with_nodelay`. Read them
  with `backlog()`, `reuse_addr()` and `nodelay()`.
- `TcpConnectionResult`: `terminal` and the `streams_*` counters are private
  (commit `dd8b57b2`). Build it with `TcpConnectionResult::new(terminal)` and
  the `with_streams_*` builders. Read it with `terminal()` and the
  `streams_*()` getters.

`TcpServerReport` keeps its public fields.

Migration: replace each struct literal with the constructor and builders, and
each field read with the getter.

### server-lifecycle: `BindConfig` and `ShutdownDeadline` fields are private

- `BindConfig` has private `host` and `port`. Build it with
  `BindConfig::new(host: IpAddr, port: u16)` (a `const fn`) or from a
  `SocketAddr` (new `From` impl). Read them with `host()` and `port()`.
- `ShutdownDeadline` has private `expires_at`, `total` and `reserve`
  (commit `df4ea7d0`). Build it with
  `ShutdownDeadline::new(expires_at, total, reserve) -> Result<Self, DeadlineError>`.
  `from_now(total, reserve)` is unchanged. Read the values with `expires_at()`,
  `total()` and `reserve()`.

Migration: replace struct literals and field reads as above.

### transport-h2c: `ManagerConfig` and `ConnectionOptions` fields are private

Every field of `ManagerConfig` (`min_connections`, `max_connections`,
`max_keepalive_connections`, `max_in_flight_per_origin`, `grow_threshold`,
`pool_timeout`, `connect_timeout`, `request_timeout`, `ping_interval`,
`idle_timeout`, `stream_window`, `conn_window`, `max_frame`) is private
(commit `e72837f2`). Each has a `with_*` builder and a getter of the same name.

`ConnectionOptions` has one field, `max_concurrent_streams`, and it is private.
It has no `with_*` builder. It is built with `ConnectionOptions::new(n)` (a
`const fn`) and read with `max_concurrent_streams()`.

Migration: start from `ManagerConfig::default()` (still exists), then chain the
builders, for example `ManagerConfig::default().with_min_connections(2).with_max_connections(8)`.
Replace each field read with the getter. For `ConnectionOptions`, replace
`ConnectionOptions { max_concurrent_streams: n }` with `ConnectionOptions::new(n)`
(`ConnectionOptions::default()` still exists), and replace
`opts.max_concurrent_streams` with `opts.max_concurrent_streams()`.

### transport-otlp: `DecodedPayload::signal()` removed

The method was deleted as unused (commit `4276e8eb`).

Migration: match on the variant. `Logs(_)` is `OtlpSignal::Logs`, `Metrics(_)`
is `OtlpSignal::Metrics`, `Traces(_)` is `OtlpSignal::Traces`, and
`Json { signal, .. }` carries its `signal`.

### peer-tls: `PeerTlsConfig` fields are private

`cert`, `key`, `ca` and `required` are private (commit `afe84d92`). Build the
config with the new `PeerTlsConfig::new(cert, key, ca, required)`, where
`cert`, `key` and `ca` take `impl Into<PathBuf>`. Read them with `cert()`,
`key()`, `ca()` and `required()` (each returns `&Path` or `bool`). The
crate-root path `peer_tls::PeerTlsConfig` still resolves.

Migration: replace the struct literal with `new` and each field read with its
getter.

### metrics-prometheus: sample, bucket and latency fields are private

- `Latency`: `sum` and `count` are private. Read them with `sum() -> &Counter`
  and `count() -> &Counter` (commit `c0c0e222`).
- `LabeledSample`: `labels` and `value` are private. Build it with
  `LabeledSample::new(labels, value)` and read with `labels()` and `value()`
  (commit `03a474b4`).
- `Bucket`, `Sample`, `Label` and `SampleGroup`: all fields are private
  (commit `3dc41c72`). Build them with `Bucket::new(le, max)`,
  `Sample::new(name, kind, help, value)`, `Label::new(name, value)` and
  `SampleGroup::new(name, kind, help, samples)`. Read them with the `const`
  getters `le()`, `max()`, `name()`, `kind()`, `help()`, `value()` and
  `samples()`.

Migration: replace each struct literal with its `new`, and each field read with
its getter. The crate-root names are unchanged.

### metrics-remote-write: `ValidatedWrite::request()` removed

The accessor was deleted as unused (commit `68d43a9f`).

Migration: use `sample_count()` to count samples without moving the request, or
`into_inner()` to take the `proto::WriteRequest`. `into_inner()` consumes the
value.

## Fixes apps must take

None. The commit subjects for these crates are refactors, tests, CI, docs and
layout moves, with no `fix:` commit. The behavior-visible changes above are the
API changes, not fixes.

## New

- `server-tcp`: `TcpHandlerError` and `TcpHandlerError::other`, the error type of `TcpHandler` futures.
- `server-tcp`: `TcpConnectionResult::new` and `with_streams_*` builders.
- `server-lifecycle`: `BindConfig::new` (const), `From<SocketAddr> for BindConfig`, `ShutdownDeadline::new`.
- `transport-h2c`: `ManagerConfig` `with_*` builders and getters; `ConnectionOptions::new` (const) and `max_concurrent_streams()`.
- `peer-tls`: `PeerTlsConfig::new` and field getters.
- `metrics-prometheus`: field getters on `Bucket`, `Sample`, `Label`, `SampleGroup`, `Latency` and `LabeledSample`.
- `server-http`: `with_*` builders and getters on `HttpServerOptions` and `TlsServerOptions`.
