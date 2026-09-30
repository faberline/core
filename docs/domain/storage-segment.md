# storage-segment

storage-segment coordinates immutable data on an object store. It models a
paged catalog, a copy-on-write tree of sorted keys whose pages are
content-addressed objects, and an archive, a set of immutable objects made
visible by writing its manifest last. Products supply the record codec and the
partitioning policy. No core crate depends on it; downstream, sift builds its
event archive and catalogs on it.

**Form:** layered, with no interfaces layer · **Depends on:** storage-object · **Crate:** [`crates/storage-segment`](../../crates/storage-segment)

## Model

- **Catalog entry** — `CatalogEntry`: a key and opaque value bytes. The
  fields are private: `CatalogEntry::try_new(key, value)` rejects a key that
  is empty, longer than 1024 bytes or contains a NUL, and `key()`, `value()`
  and `into_parts()` read it. Deserializing does not check the key; the
  catalog checks every key when it builds or loads a page.
- **Catalog page** — a JSON leaf of entries or branch of child references.
  `CatalogPageRef` names a page by key and records its sha256, size, entry
  count and first and last keys.
- **Catalog root** — `CatalogRoot`: format version, tree height, entry count,
  page size limit and the root page reference. A catalog is a value: every
  change yields a new root.
- **Catalog mutation** — `CatalogMutation`: the new root, the page keys
  written, and the keys of the old pages it replaced.
- **Streaming build** — `StreamingCatalogBuild` reports a bulk build over
  sorted input; `StreamingCatalogAbort` returns the error and every page key
  a failed build may have written, for cleanup.
- **Archive object** — `ArchiveObject` (key, bytes, content type), written as
  an `ArchivedObject` receipt with size, sha256 and object version.
  `ArchivedObjectVersion` is the store's version as a bare JSON string;
  infrastructure converts storage-object's `ObjectVersion` into it.
- **manifest** — the last object of an archive transaction. The archive
  commit writes it after every object it names; `ArchiveCommit` is the receipt.
- **Segment error** — `SegmentError`: codec, partition, catalog, transaction
  and object-store failures. `SegmentError::ObjectStore` boxes the
  underlying error; infrastructure converts storage-object's
  `ObjectStoreError` into it, and the message is that error's own.

## Ports

- `RecordCodec<Record>` — the product codec for records in one immutable
  segment. Implemented by sift.
- `Partitioner<Record>` — the product policy that picks a stable partition
  for a record. Implemented by sift.
- `ImmutableObjectStore` (crate-internal) — write-once storage for catalog
  pages and archive objects, plus page reads and cleanup deletes. The
  infrastructure adapter implements it over a storage-object `ObjectStore`.

`PagedCatalog` and `ArchiveCoordinator` hold the `ImmutableObjectStore` port.
Their public constructors, `PagedCatalog::new`,
`PagedCatalog::with_page_bytes` and `ArchiveCoordinator::new`, take an
`Arc<dyn ObjectStore>`; they live in the composition root (`src/app`), which
wraps the store in the adapter. The page codec and the SHA-256 content hash
are pure domain functions under `domain/catalog`.

## Invariants

- A page's key is `<prefix>/pages/<sha256 of its JSON>.json`, written only if
  absent; an existing key with other bytes is `ImmutableObjectChanged`.
- Loading a page checks its size, sha256, format version, entry count and
  first and last keys against the reference.
- `upsert` and `remove` rewrite only the pages on the key's search path.
  Removing a missing key returns the original root and writes nothing.
- `build_sorted` rejects an equal or decreasing key and holds at most one
  page-sized buffer per tree level. A page limit is 4–64 KiB (default 64 KiB).
- A catalog key is non-empty, at most 1024 bytes and free of NUL.
- Archive objects are written only if absent. An object already stored under
  the key is accepted only when its bytes and content type are identical.
- A key appears once per archive transaction, and the manifest may not reuse
  an object's key. After any failed write the transaction refuses further
  puts and its commit.

## Published language

No core context depends on storage-segment. sift imports from the crate root:
`PagedCatalog`, the catalog types, `ArchiveCoordinator`, `ArchiveTransaction`,
`ArchiveObject`, `RecordCodec`, `Partitioner`, `SegmentError` and `Result`.
Every export is at the crate root; sift's structure test checks for the
exact path `storage_segment::RecordCodec`.

## Exceptions and debts

- **Checker exceptions:** none.
- **Debts:** none tracked. P2 deleted the unimplemented `SegmentStore`
  trait (D7) and made the `CatalogEntry` fields private (D2).
