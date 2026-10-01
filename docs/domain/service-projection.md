# service-projection

service-projection keeps durable, rebuildable read models over an ordered
source of records. A product defines its records, its source and each
projection's logic; this context owns checkpoints, catch-up, rebuild,
publication and flush. A projection is never the source of truth: it is
rebuilt from the source when its saved state is unreadable or the source
generation changes. No core crate depends on it; downstream, sift keeps its
log, metric and trace projections with it.

**Form:** layered · **Depends on:** storage-durable · **Crate:** [`crates/service-projection`](../../crates/service-projection)

## Model

- **Identity types** — `ProjectionName` and `ProjectionEventId` wrap strings
  (`new`, `as_str`). `ProjectionCursor` and `SourceGeneration` wrap separate
  numbers (`new`, `get`). The record and source ports, descriptors, checkpoints,
  lag reports and runtime cursor methods use these types. JSON, saved state
  and the three published OpenAPI schemas keep their exact bytes.

- **Projection descriptor** — `ProjectionDescriptor`: a name, a schema
  version and a retention label that the runtime does not interpret. The
  fields are private: `ProjectionDescriptor::try_new(name, schema_version,
  retention)` returns `InvalidName` for an invalid name, and `name()`,
  `schema_version()` and `retention()` read it. Deserializing does not check
  the name; the runtime checks it again when it opens a projection.
- **Projection cursor** — the `u64` cursor of the last source record applied.
- **Source generation** — the generation of the retained source a projection
  was built from. A source bumps it when retention or repair removes or
  replaces records without moving its cursor high-water mark.
- **Projection checkpoint** — `ProjectionCheckpoint`: name, schema version,
  cursor, source generation, last event id, state sha256 and update time.
  The domain does not read the clock: `ProjectionCheckpoint::empty` and the
  checkpoint builder take `now: DateTime<Utc>`, which the runtime reads, and
  write it as RFC 3339 UTC with milliseconds (`2026-01-02T03:04:05.678Z`).
- **Projection snapshot** — the state bytes, saved in a
  `ProjectionStateEnvelope` with a format version and the checkpoint.
- **Projection quarantine** — a state file that failed to load, renamed aside.
- **Projection lag** — `ProjectionLag`: the retryable error `projection_lag`
  with the required and current cursors and a retry-after in seconds.
- **Rebuild comparison** — `RebuildComparison`: the live and rebuilt semantic
  digests at one source cursor, and whether they are equal.
- **Projection error** — `ProjectionError`: `InvalidName` ("projection name
  is invalid") and `Other`, which wraps an implementation's error with its
  message unchanged. The registry and handle keep returning `anyhow::Result`.
- **Registry and handle** — `ProjectionRegistry` binds one source to named
  projections; `ProjectionHandle` is the typed handle of one.
  `ProjectionRuntimeConfig::new` raises batch size, snapshot interval and
  retry-after to at least 1.

## Ports

sift implements all four. Every fallible method returns
`Result<_, ProjectionError>`; an implementation wraps its own error, such as
an `anyhow::Error`, with `ProjectionError::other`.

- `ProjectionRecord` — a source record's cursor and event id.
- `ProjectionSource<Record>` — the current cursor, `read_after`, and
  `generation` (0 by default).
- `ProjectionReadSession<Record>` — an optional forward-only scan a source
  opens for a whole catch-up or rebuild.
- `Projection<Record>` — the descriptor, `apply_idempotent`, `snapshot`,
  `restore`, `checkpoint_committed` and `semantic_digest` (by default the
  sha256 of the snapshot).

The runtime saves state through one crate-internal port, which products do
not see:

- `ProjectionStateStore` — `prepare_root`, `read` a projection's saved
  bytes, `restore` (decode and check them against the descriptor), `quarantine`
  and `persist`. Infrastructure implements it with the envelope file of each
  projection. The composition root (`src/app`) keeps the public
  `ProjectionRegistry::new(root, source, config)`: it builds the file store
  under `root` and passes it to the registry, which hands it to every handle.

## Invariants

- A name is not blank and has no `/` or NUL; a registry refuses a duplicate.
- State is saved as `indexes/<name>/state.json` under the registry root with
  `atomic_write` under `FsyncPolicy::Always`; on Unix, directories are 0700
  and files 0600. `checkpoint_committed` runs only after the save.
- A state file is restored only if its format version is
  `PROJECTION_STATE_FORMAT_VERSION` (1), its name and schema version match,
  its encoding is base64 and its state matches the checkpoint's sha256.
  Otherwise it becomes `state.corrupt-<16 hex digits>.json` and the
  projection is rebuilt.
- On a source generation change, opening, catch-up, flush and
  `rebuild_and_compare` rebuild the projection from the start.
- Catch-up applies records only up to the source cursor read when it starts.
  It saves on the first advance, or once the cursor is
  `snapshot_interval_events` past the last save; `flush` saves the rest.
- `rebuild_and_compare` replaces the live projection only when the digests
  are equal; `rebuild` replaces it either way.
- `wait_for_min_cursor` returns once the cursor reaches the required value,
  or `ProjectionLag` at the timeout.

## Published language

No core context depends on service-projection. sift re-exports
`ProjectionCheckpoint`, `ProjectionDescriptor`, `ProjectionLag`,
`ProjectionStateEnvelope`, `RebuildComparison` and
`PROJECTION_STATE_FORMAT_VERSION`, and its API error carries a
`ProjectionLag`. The envelope is a persisted format. Every export is at the
crate root; there is no old module path. sift's structure test checks its
own sources for `service_projection::ProjectionRegistry`.

## Exceptions and debts

- **Checker exceptions:**
  - B2 (`utoipa`), long-term: `ProjectionDescriptor`, `ProjectionCheckpoint`
    and `ProjectionLag` derive `ToSchema`, a compile-time description of the
    same serde wire shape with no I/O. sift serves them as-is in its OpenAPI
    document under these schema names, so an interfaces copy would duplicate
    the wire contract.
- **Public fields kept:** `ProjectionCheckpoint`, `ProjectionLag` and
  `RebuildComparison` are built only inside this crate;
  `ProjectionStateEnvelope` is the persisted state format.
  `ProjectionRuntimeConfig` has public fields and a `new` that raises each
  value to at least 1.
- **Debts:**
  - `anyhow` in the `ProjectionRegistry` and `ProjectionHandle` methods
    (these are not ports, so ADR D4 does not cover them).

  P2 made the projection ports return `ProjectionError` (D4), and made
  the `ProjectionDescriptor` fields private behind `try_new` (D2).

P2 typed projection names, event ids, cursors and source generations (W5).
