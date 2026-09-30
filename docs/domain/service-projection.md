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

- **Projection descriptor** — `ProjectionDescriptor`: a name, a schema
  version and a retention label that the runtime does not interpret.
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
`ProjectionLag`. The envelope is a persisted format that P1 does not change.
P1 keeps every root export; there is no old module path. sift's structure
test checks its own sources for `service_projection::ProjectionRegistry`.

## Exceptions and debts

- **Checker exceptions (P1):**
  - B2 (`utoipa`): `ProjectionDescriptor`, `ProjectionCheckpoint` and
    `ProjectionLag` derive `ToSchema`, and sift's OpenAPI document uses those
    schema names. P2 moves the schemas to interfaces types with the same names.
  - B3 `application->infrastructure`: `ProjectionHandle` and
    `ProjectionRegistry` call the file-state functions directly. P2 adds a
    state-store port.
- **Tracked for P2:**
  - `ProjectionDescriptor` built with struct literals by sift (ADR D2); the
    checkpoint, envelope, lag and comparison types also have public fields.
  - Bare ids: cursors and generations are `u64`, names and event ids `String`.
