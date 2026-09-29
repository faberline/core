# service-executor

service-executor supplies application-neutral, bounded asynchronous
execution: a concurrency-limited runner, a group-commit batcher and a job
runner. It owns concurrency mechanics only. Durable assignment, fencing,
retry, external target semantics and outcome persistence stay in each
domain's committed state; the caller must hold ownership of the work, and
permission for any external effect, before submitting it. No core crate
depends on it; downstream, defer runs dispatch through `run_bounded`, and
sift uses the group commit and the job runner.

**Form:** whole-src infrastructure · **Depends on:** — · **Crate:** [`crates/service-executor`](../../crates/service-executor)

## Model

- **Bounded run** — `run_bounded`: every input executed with at most a set
  number of futures in flight.
- **Group-commit request** — a domain request that flattens into items of one
  shared batch. The domain owns its key, item count and byte size.
- **Group-commit batch** — requests with the same key executed together by a
  single call of the batch function, whose outputs are fanned back out.
- **Group-commit config** — `GroupCommitConfig`: the maximum delay, items and
  bytes per batch, and the queue capacity (1024 by default).
- **Queue and worker** — `spawn_group_commit` returns a cloneable
  `GroupCommitQueue` for submitting and a `GroupCommitWorker` to join at
  shutdown.
- **Group-commit error** — `GroupCommitError`: `Closed`, `EmptyRequest`,
  `RequestTooLarge`, `OutputCount`, or `Sink` with the batch function's error.
- **Job run** — one job executed by `JobRunner`, blocking
  (`spawn_blocking`) or asynchronous (`spawn_async`), with its state
  transitions recorded through `JobState`.
- **Job run report** — `JobRunReport`: `JobRunState` `Succeeded`, `Failed` or
  `PersistenceFailed`, and the persistence error, if any.

## Ports

- `GroupCommitRequest` — the key, item count, encoded byte size and items of
  a request. Implemented by sift.
- `JobState<O>` — records `mark_running`, `succeed` and `fail` for a job id,
  with its own error type. Implemented by sift.

## Invariants

- `run_bounded` returns every result, in completion order; a concurrency of
  0 is treated as 1.
- `GroupCommitConfig` requires every limit to be non-zero. A submit with no
  items is `EmptyRequest`, and one over either limit on its own is
  `RequestTooLarge`; neither is queued.
- A batch starts with the first waiting request and closes at `max_delay`
  after it, or when the next request has another key or would pass the item
  or byte limit; that request starts the next batch.
- Each request receives exactly as many outputs as it has items, in order.
  If the batch function returns a different count, every request gets
  `OutputCount`; if it fails, every request gets the same `Sink` error.
- Dropping every queue handle closes the worker after it drains the requests
  already accepted.
- A job runs only after `mark_running` succeeds. An error or a panic is
  recorded with `fail` (a panic as "job execution panicked"); a failed state
  write makes the run `PersistenceFailed`.

## Published language

The whole public API; whole-src infrastructure contexts have no application
layer. Every export is at the crate root; the group-commit and job-runner
modules are private, so no old module path needs a facade. sift's structure
tests check its own sources for the exact paths
`service_executor::GroupCommitRequest` and `service_executor::JobRunner::new`.

## Exceptions and debts

- **Checker exceptions (P1):** None. Task spawning and timers are the job of
  an infrastructure crate.
- **Tracked for P2:** None found. `GroupCommitConfig` and `JobRunReport`
  expose public fields, but no downstream struct literal was found; sift
  builds the config with `GroupCommitConfig::new`.
