# service-collector

service-collector is a product-neutral, at-least-once collector runtime. A
product provides the source, the record decoder and the delivery sink; this
context owns batching, delivery retry, quarantine ordering and the order in
which source cursors are committed. A source's cursors are committed only
after their batch is delivered and its rejections are quarantined, so a
failure leaves them uncommitted. No core crate depends on it;
downstream, sift runs its collectors on it.

**Form:** layered, with no interfaces layer · **Depends on:** storage-durable · **Crate:** [`crates/service-collector`](../../crates/service-collector)

## Model

- **Collector cursor** — the source's own `Cursor` type: where a record sits
  in the source. Every record and every rejection carries one.
- **Read outcome** — `ReadOutcome`: a `Record`, a `Rejection`, `Pending` or
  `Exhausted`.
- **Rejection** — an entry that will not be delivered. The source rejects a
  record it cannot read (`CollectorRejection`), or the decoder rejects a
  record it cannot decode; both become quarantine entries.
- **Collector quarantine** — where rejection entries go. `JsonlQuarantine`
  appends them to a JSONL file.
- **Collector checkpoint** — the product's own resume state, stored as JSON
  with `save_json_checkpoint` and read with `load_json_checkpoint`.
- **Delivery receipt** — `DeliveryReceipt`: how many records of a batch the
  sink accepted and how many it saw as duplicates.
  `DeliveryReceipt::new(accepted, duplicates)` builds it, and `accepted()`
  and `duplicates()` read it.
- **Delivery failure** — `DeliveryFailure`: a message that is either
  `retryable` or `permanent`.
- **Source commit** — acknowledging the cursors of a batch to the source,
  with `CommitStats` (accepted, duplicates, rejected).
- **Source progress** — `SourceProgress`: start and final offsets and the
  bytes and sources the source reports lost. `SourceProgress::new(start_offset,
  final_offset, lost_bytes, lost_sources)` builds it, and getters named after
  the fields read it.
- **RuntimeConfig** — the collector's `RuntimeConfig`: batch size, record
  byte limit, `RetryPolicy`, whether to follow the source, and the follow
  poll interval. The fields are private: `RuntimeConfig::try_new(batch_size,
  max_record_bytes, retry, follow, follow_poll_interval)` builds it, and
  getters named after the fields read it. `DeliveryRetryMode` is `Bounded`
  or `UntilCancelled`.
- **Run report** — `RunReport`: lines read, accepted, duplicates, rejected,
  and the final progress.

## Ports

- `CollectorSource` — reads records up to a byte limit, commits cursors, and
  optionally refreshes and reports progress. Errors are the source's own
  `Error` type. Implemented by sift.
- `RecordDecoder<R>` — turns a source record into a deliverable item or a
  rejection entry. Implemented by sift.
- `BatchSink<T>` — asynchronous delivery of one batch; returns a
  `DeliveryReceipt` or a `DeliveryFailure`. Implemented by sift.
- `QuarantineSink<T>` — appends rejection entries; `JsonlQuarantine`
  implements it.

## Invariants

- Each batch is delivered first, then its rejections are appended to the
  quarantine, then all its cursors, records and rejections alike, are
  committed to the source. Any failure ends the run before the commit.
- A success receipt must cover the batch exactly: accepted plus duplicates
  equals the number of records. Otherwise the run fails without committing,
  in both retry modes. A batch with no records skips the sink.
- A permanent failure ends the run at once. Under `Bounded`, a retryable
  failure is retried up to `max_retries` times; under `UntilCancelled`, the
  same batch is retried until it succeeds or the run is dropped.
- The retry delay doubles from `initial_backoff` per attempt, at most 64
  times the initial value, and never exceeds `max_backoff`.
- Batch size, record byte limit, follow poll interval and initial backoff
  must be positive, and `max_backoff` not below `initial_backoff`.
  `RuntimeConfig::try_new` checks its three limits when the config is built;
  the runtime checks the `RetryPolicy` again before the first read.
- `Pending` or `Exhausted` ends the batch. With `follow`, the runtime sleeps
  one poll interval and refreshes the source; without it, the run ends.
- `JsonlQuarantine` fsyncs before it returns. A JSON checkpoint is saved with
  `atomic_write` under `FsyncPolicy::Always`. Both files get a private mode.

## Published language

No core context depends on service-collector. sift implements
`CollectorSource`, `RecordDecoder`, `BatchSink` and the record and rejection
traits, and uses `run_collector_with_delivery_mode`, `JsonlQuarantine`, the
JSON checkpoint and JSONL helpers and the model types, all from the crate
root. P1 keeps every root export; there is no old module
path to keep. sift's structure test checks its own sources for the exact
paths `service_collector::run_collector`, `service_collector::RecordDecoder`,
`service_collector::BatchSink` and `service_collector::save_json_checkpoint`.

## Exceptions and debts

- **Checker exceptions (P1):** None. `BatchSink` is declared with
  `#[async_trait]`, which is not on the domain allowlist, so it sits in the
  application layer; the runtime and the file helpers are outside the domain.
- **Tracked for P2:**
  - Bare ids: offsets and counters are `u64`.
- **Public fields, not changed in P2:** `CommitStats` and `RunReport` are
  built only inside this crate. `RetryPolicy` has public fields, but sift
  builds it with `RetryPolicy::new`, and the runtime checks it again before
  the first read.
