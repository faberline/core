# storage-durable

storage-durable owns the repeated mechanics of durable local files: the fsync
policy, crash-safe atomic replacement, CRC-framed append logs, sequence-named
snapshot files, generation directories switched by a `CURRENT` pointer, a
locked data root and disk-capacity admission. Services keep their own record
and snapshot codecs. In core, raft-runtime, service-backup, service-collector,
service-projection and storage-object build on it; downstream, beam, defer,
lumen, sift and tape use it directly.

**Form:** whole-src infrastructure · **Depends on:** — · **Crate:** [`crates/storage-durable`](../../crates/storage-durable)

## Model

- **Fsync policy** — `FsyncPolicy`: `Always` (the default), `EverySec`,
  `Interval` or `Os`. `atomic_write` fsyncs under every policy except `Os`;
  the framed-log writer's `maybe_sync` acts on `EverySec` alone, with a fixed
  one-second interval.
- **Atomic write** — `atomic_write`: write `<path>.tmp`, fsync it, rename it
  over the target, fsync the parent directory. The `_strict` variants never
  treat a directory that cannot be opened for fsync as success.
- **Framed log** — `FramedLogWriter` and `FramedLogReader`: frames of a
  16-byte header (`seq`, `len`, CRC-32 of the payload) and the payload, read
  back as `LogFrame` or memory-mapped `MappedLogFrame`.
- **Snapshot file** — `SnapshotFile`: a file named `{prefix}-{seq}.{ext}` in
  a `SnapshotFileStore`.
- **Directory generation** — an immutable child directory of a
  `GenerationStore`, named by a `GenerationName`; a generation commit switches
  `CURRENT` to one of them. The caller owns the bytes and their validation.
- **Data root** — `DataRoot`: a service's private data directory with a
  layout manifest, held under an exclusive lock.
- **Capacity** — `CapacityGuard` tracks used and reserved bytes against a
  maximum and a free-space floor. `CapacityLevel` is `Normal`, `Warning`,
  capacity backpressure (`Backpressure`) or `Critical`, set by
  `CapacityThresholds` (70/80/90 by default).

## Ports

- `DataRootPolicy` — a service's manifest and compatibility hooks: product
  name, manifest file, directories, legacy markers, manifest creation and
  validation. Implemented by sift.
- `SpaceProbe` — free space under a root; `FileSystemSpaceProbe` implements
  it.

## Invariants

- A crash during `atomic_write` leaves the old bytes or the new ones, never a
  prefix. One writer per path is a precondition, because the temp name is fixed.
- The first frame that fails a check (payload limit, end of file, CRC) ends
  the log; opening a writer truncates the file to the last good frame.
- `truncate_through` keeps frames with a higher `seq` and rewrites the whole
  log through a temp file.
- The sequence in a snapshot file's name is the only ordering trusted; a name
  that does not parse is skipped, and `prune` keeps the newest `keep` files.
- Renaming `CURRENT.tmp` over `CURRENT` is the only generation commit point.
  A `GenerationName` is a safe direct-child name, so it cannot escape the
  store's root or name `CURRENT` itself.
- `DataRoot::open` rejects symlinks, refuses a legacy root without a manifest,
  and fails if another process holds the root's lock.
- `CapacityThresholds` require `warning < backpressure < critical <= 100`. An
  admission fails when projected use reaches the backpressure percentage or
  free space would drop below the floor; an uncommitted reservation is
  released on drop.

## Published language

The whole public API; whole-src infrastructure contexts have no application
layer. Every export is at the crate root and has no old module path to keep.

## Exceptions and debts

- **Checker exceptions (P1):** None. File-system access is the job of an
  infrastructure crate.
- **Tracked for P2:**
  - `anyhow` in the `DataRootPolicy` port and in `DataRoot` (ADR D4); sift
    implements the port.
  - Public fields built with struct literals (ADR D2): `LogFrame`, in lumen's
    tests. `SnapshotFile`, `MappedLogFrame` and `CapacityError` also expose
    public fields.
