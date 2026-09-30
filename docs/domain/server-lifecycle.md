# server-lifecycle

server-lifecycle models the life of a server process, independent of any
protocol: the phases it moves through, whether it admits new work, the ordered
shutdown that drains it within a deadline, and the cap on concurrent
connections. It owns no listener and no wire format. In core, server-tcp,
server-http, transport-h2c, service-http, service-observability and
raft-runtime build on it; downstream, pgpool, lumen, mamba, sift and beam use it
directly, and every service that uses service-http reaches it through that
crate.

**Form:** whole-src infrastructure · **Depends on:** — · **Crate:** [`crates/server-lifecycle`](../../crates/server-lifecycle)

## Model

- **Lifecycle phase** — `LifecyclePhase`: `Starting`, `Recovering`, `Serving`,
  `Degraded`, `Draining`, `Stopping`, `Stopped`, `Fatal`.
- **Lifecycle observation** — `LifecycleObservation`: the current phase, its
  lifecycle generation, when it was entered, a reason code and detail, and
  lifecycle admission (`admission_open`). `is_ready` is lifecycle admission;
  `is_healthy` is false only in `Stopped` and `Fatal`.
- **Lifecycle controller** — `LifecycleController`: the shared, cloneable owner
  of the phase. Observers follow it with `subscribe` (latest value) or
  `subscribe_events` / `subscribe_transitions` (every transition, bounded
  buffer, lag reported).
- **Shutdown deadline** — `ShutdownDeadline`: an expiry, a total budget and a
  reserve held back for final work. Usable time is the remaining time minus the
  reserve.
- **Shutdown hook** — a named async step registered at a `HookStage`:
  `AdmissionStop`, `TransportDrain`, `DomainQuiesce`, `BackgroundStop`,
  `FinalFlush`. Each run yields a `HookOutcome` (`Completed`, `Failed`,
  `TimedOut`), collected in a `ShutdownReport` with `PhaseTiming`s.
- **Drain signal** — `DrainController` / `DrainSignal` (`DrainState`): the older
  ready-or-draining flag, kept for callers that predate the lifecycle.
- **Readiness** — `Readiness`: "is this process draining?", answered by the
  controller, a subscription, or a drain signal.
- **Connection budget** — `ConnectionBudget`: connection admission, the cap on
  concurrent connections. `try_acquire` returns a `ConnectionPermit` or
  `ConnectionLimitExceeded`.
- **Task supervisor** — `TaskSupervisor`: named tasks and hooks shut down over
  one controller and deadline, with their errors collected.
- **Bind config** — `BindConfig`: the host and port a server binds, built with
  `new` or from a `SocketAddr`.

## Ports

- `ConnectionMetrics` — accepted, rejected and closed connection events, with
  no-op defaults. `NoopConnectionMetrics` is the default; service-observability's
  `LifecycleMetrics` implements it.
- `Readiness` — implemented here for the controller, `LifecycleSubscription`,
  `DrainController` and `DrainSignal`. service-http re-exports it as
  `ReadinessHook`; downstream services implement that name on their app state.

## Invariants

- A transition follows only these edges: `Starting` to `Recovering`,
  `Serving`, `Degraded`, `Draining` or `Fatal`; `Recovering` to `Serving`,
  `Degraded`, `Draining` or `Fatal`; `Serving` to `Degraded`, `Draining` or
  `Fatal`; `Degraded` to `Serving`, `Draining` or `Fatal`; `Draining` to
  `Stopping` or `Fatal`; `Stopping` to `Stopped` or `Fatal`. Any other edge is
  `LifecycleError::InvalidTransition`; a transition to the current phase is a
  no-op.
- Every transition bumps the lifecycle generation by one. Lifecycle admission
  is open only in `Serving` and `Degraded`, and `transition_degraded` sets it
  explicitly for a degraded process.
- `shutdown` runs once. It publishes the deadline before entering `Draining`,
  closes hook registration (`RegistrationClosed` afterwards), runs hooks in
  stage then registration order, and moves through `Stopping` to `Stopped`.
  Every later caller gets the same `ShutdownReport`.
- Each hook runs in its own task, bounded by the usable time left. A panic is
  recorded as `Failed`; a timeout aborts the task and records `TimedOut`; a hook
  that starts with no usable time left is `TimedOut`.
- `ShutdownDeadline::new` and `from_now` reject a reserve larger than the
  total (`ReserveExceedsTotal`).
- A `ConnectionBudget` allows at least one connection, and a
  `ConnectionPermit` returns its slot when dropped.

## Published language

The whole public API; whole-src infrastructure contexts have no application
layer.

## Exceptions and debts

- **Checker exceptions (P1):** None. Signals, the clock and task spawning are
  the job of an infrastructure crate.
- **Tracked for P2:** None.
