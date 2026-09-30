# server-tcp

server-tcp is the generic TCP accept loop. A protocol implements `TcpHandler`
for one connection; this crate owns the listener loop, connection admission
through a `ConnectionBudget`, the per-connection tasks and what happens to them
during a drain. It knows nothing of HTTP. server-http builds its listener on
it; downstream, the pgpool Postgres pooler uses it directly with its own
handlers.

**Form:** whole-src interfaces · **Depends on:** server-lifecycle · **Crate:** [`crates/server-tcp`](../../crates/server-tcp)

## Model

- **TCP server config** — `TcpServerConfig`: the `BindConfig`, a connection
  budget, a legacy `DrainController`, socket options, a drain timeout (5 s by
  default) and a `ConnectionMetrics` sink, set through `with_*` builders.
- **Socket options** — `TcpSocketOptions`: listen backlog (1024 by default),
  address reuse and `TCP_NODELAY`, changed with `with_*` builders.
- **Connection context** — `ConnectionContext`: what a handler learns about its
  connection: local and peer address, a `DrainSignal` and a
  `LifecycleSubscription`, so a protocol can drain its own streams.
- **Connection result** — `TcpConnectionResult`: how one connection ended
  (`TcpConnectionTerminal`: `Completed`, `Failed`, `TimedOut`) and its stream
  counts (admitted, active at drain, completed, refused, timed out,
  ambiguous), built with `new(terminal)` and `with_*` counters by a protocol
  that multiplexes streams.
- **Server report** — `TcpServerReport`: the totals for one `serve_with_report`
  run: connections accepted, rejected, completed, failed, timed out and
  unfinished, the summed stream counts, accept errors, and `deadline_missing`.

## Ports

- `TcpHandler` — serve one accepted `TcpStream` with its `ConnectionContext`;
  its future resolves to `anyhow::Result<()>`. A blanket impl covers closures.
  pgpool implements it for its session, transaction and pool handlers.

## Invariants

- The loop stops accepting once the lifecycle reaches `Draining` or later.
  Under `serve` and `serve_arc` the lifecycle is the one behind the configured
  `DrainController`, and the shutdown future starts its drain. A connection
  accepted after the drain began is closed and counted as rejected.
- A connection that finds the budget full is closed, counted as rejected and
  reported to `ConnectionMetrics`; it never reaches the handler.
- Under `serve_with_report`, the drain waits until the deadline the given
  lifecycle published. If none was published, the report sets
  `deadline_missing` and every connection task is aborted. Under `serve` and
  `serve_arc`, the configured drain timeout applies instead.
- Connection tasks still running at the deadline are aborted and counted as
  unfinished.
- Under `serve` and `serve_arc`, a handler error only marks that connection
  `Failed`; it does not stop the loop.

## Published language

The whole public API; whole-src interfaces contexts have no application layer.
Its entry points are `bind`, `serve`, `serve_arc` and `serve_with_report`.

## Exceptions and debts

- **Checker exceptions (P1):** None.
- **Tracked for P2:** `anyhow` in the `TcpHandler` port (ADR D4).
