# transport-h2c

transport-h2c is the h2c transport between faberline services: HTTP/2 over
cleartext TCP with prior knowledge, no TLS and no ALPN. The client side sizes
and manages connections to one authority; the server side, behind the `server`
feature, serves one already-accepted stream as HTTP/1.1 or h2c and drains it
under a server-lifecycle deadline. It never binds a listener. In core,
server-http and raft-runtime use it. Downstream, keep and lumen use the client
side in production code; relay, tape and pgpool use it in tests and examples,
and lumen's tests also serve connections with it.

**Form:** whole-src infrastructure · **Depends on:** cli-std, server-lifecycle · **Crate:** [`crates/transport-h2c`](../../crates/transport-h2c)

## Model

- **Connection-count heuristic** — `recommended_h2c_connections`:
  `clamp(ceil(ln concurrency), 1, cpu_parallelism)`, because one h2 connection
  serializes its framing on one core.
- **h2c client** — `h2c_client` / `h2c_client_with`: a reqwest client that
  speaks HTTP/2 with prior knowledge.
- **h2c pool** — `H2cPool`: a fixed set of such clients used round-robin.
- **h2c manager** — `H2cManager`: a self-managing set of frame-level h2
  connections to one authority, configured by `ManagerConfig` and observed
  through `ManagerStats`.
- **Stream admission** — the manager's per-origin semaphore
  (`max_in_flight_per_origin`): a request waits up to `pool_timeout` for a slot
  instead of opening unbounded streams.
- **Connection lease** — a checked-out connection from the manager, released
  when the request finishes.
- **h2c error** — `H2cError`. `Refused` and `Ambiguous` tell a caller whether a
  lost mutation might have run.
- **Connection terminal** — `ConnectionTerminal` (`PeerClosed`, `Drained`,
  `DeadlineExceeded`, `Failed`) and the `ConnectionReport` of one served
  connection; `ConnectionOptions` caps concurrent streams (4096 by default).

## Ports

None.

## Invariants

- The manager sends each request on the healthy connection with the fewest
  in-flight streams. It opens a new connection when the least-loaded one is
  saturated, up to `max_connections`. A supervisor pings connections, evicts
  dead ones, shrinks idle ones and replenishes to `min_connections`.
- A safe method (GET, HEAD, OPTIONS, TRACE) that loses its connection is
  retried once on another connection. A lost non-safe request is never
  retried: it fails as `Refused` when the peer refused the stream, else as
  `Ambiguous`.
- After `shutdown`, every request fails with `Shutdown`.
- While a served connection drains, a new request is answered 503 with
  `connection: close` and counted as refused. In-flight requests get the
  lifecycle deadline's usable time; if it expires, the connection ends as
  `DeadlineExceeded` and the requests still running are counted as timed out.
- A drain that finds no published deadline ends the connection as `Failed`.
- A dropped in-flight mutation is counted as ambiguous, never as completed.

## Published language

The whole public API; whole-src infrastructure contexts have no application
layer. The `server` module is also reached by path: `serve_io*` is not
re-exported at the root, so server-http calls `server::serve_io_with_options`,
and raft-runtime and lumen tests call `server::serve_connection`. lumen reads
the crate's llm topic through `llm::topic`.

## Exceptions and debts

- **Checker exceptions (P1):** None.
- **Tracked for P2:** the safe-method check exists twice, `is_safe_method` in
  both the manager and the server side (ADR D7). server-http builds
  `ConnectionOptions` with struct literals.
