# server-http

server-http is the HTTP listener shell: one port that serves HTTP/1.1 and h2c
to an axum `Router`, or terminates TLS first. It composes server-tcp's accept
loop with transport-h2c's per-connection serving under a server-lifecycle
drain. Route and middleware policy, the service probes and certificate
handling are not here: service-http sits above it, and peer-tls supplies
certificate material through a closure. In core, service-http uses it;
downstream, pgpool serves its admin port with it.

**Form:** whole-src interfaces · **Depends on:** server-lifecycle, server-tcp, transport-h2c · **Crate:** [`crates/server-http`](../../crates/server-http)

## Model

- **HTTP server options** — `HttpServerOptions` (older name
  `H2cServerOptions`): the HTTP/2 stream cap per connection (4096 by default),
  a drain timeout (5 s), an optional `ConnectionBudget`, a `DrainController`,
  `TcpSocketOptions` and a `ConnectionMetrics` sink, set through `with_*`
  builders on `Default` (`set_drain_timeout` changes a built value).
- **HTTP server report** — `HttpServerReport`: the connection and stream totals
  of one lifecycle-driven run. It is a type alias of server-tcp's
  `TcpServerReport`.
- **Server config source** — `ServerConfigSource`: a type alias for a shared
  closure that returns the rustls `ServerConfig` active right now, or `None`
  when nothing valid is active. `config_source` wraps a closure as one.
- **TLS server options** — `TlsServerOptions`: `HttpServerOptions` plus the
  listener's `TlsListenerMetrics`, set with `with_http` and `with_metrics`.
- **Metrics snapshot** — `TlsListenerSnapshot`: handshakes established,
  handshake failures, and connections refused because no material was active.
- **server-http trace layer** — `trace_layer`: a tower-http request trace layer
  at INFO. It is not service-http's `trace_layer`.

## Ports

None. The TLS seam is `ServerConfigSource`, a closure type rather than a trait;
peer-tls's `ReloadableTls` is one way to supply it.

## Invariants

- `serve_h2c_with_lifecycle` is the production path: the lifecycle it is given
  owns the drain and its deadline. Each connection's transport-h2c terminal
  maps to a server-tcp one: a missed deadline to `TimedOut`, a failure to
  `Failed`, a peer close or a clean drain to `Completed`.
- `serve_h2c` and `serve_h2c_with_options` are legacy adapters driven by a
  shutdown future and a relative drain timeout.
- `serve_tls` reads the config source once per accepted connection. A
  connection keeps the configuration it started with, so a rotation needs no
  rebind or restart.
- When the config source returns `None`, the connection is refused and counted;
  it is never served in cleartext.
- ALPN comes from the supplied `ServerConfig`; this crate never sets it and
  never parses PEM or judges identity.

## Published language

The whole public API; whole-src interfaces contexts have no application layer.
The crate root also re-exports server-lifecycle as `core` and server-tcp as
`tcp`.

## Exceptions and debts

- **Checker exceptions (P1):** None.
- **Tracked for P2:** server-http builds transport-h2c's `ConnectionOptions`
  with struct literals.
