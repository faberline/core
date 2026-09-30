# storage-object

storage-object is the object I/O boundary for local durable storage and cloud
archives. It models an object with versioned metadata and conditional writes,
and ships three adapters: a local directory, Google Cloud Storage (the default
`gcs` feature) and S3-compatible stores (the optional `s3` feature). In core,
service-backup and storage-segment depend on it; downstream, sift uses the GCS
adapter and implements the port itself.

**Form:** whole-src infrastructure · **Depends on:** storage-durable · **Crate:** [`crates/storage-object`](../../crates/storage-object)

## Model

- **Object key** — a `/`-separated path relative to the store's root
  directory or bucket prefix.
- **Object version** — `ObjectVersion`: an opaque version string for
  conditional writes. Its meaning is per adapter: the content sha256 for the
  local store, the object generation for GCS, and the version id, else the
  ETag, for S3.
- **Object metadata** — `ObjectMeta`: key, size, content type, version, and
  the optional ETag and update time. The fields are private:
  `ObjectMeta::new(key, size, content_type, version)` builds it, with
  `with_etag` and `with_updated` for the optional fields, and getters named
  after the fields read it. `Object` is metadata plus bytes:
  `Object::new(meta, bytes)`, `meta()`, `bytes()` and `into_parts()`. The
  serde shape of `ObjectMeta` is unchanged.
- **Put condition** — `PutCondition`: `Any`, `IfAbsent` or
  `IfVersion(version)`.
- **Object-store error** — `ObjectStoreError` (thiserror): `NotFound`,
  `PreconditionFailed`, `InvalidKey`, `UnsafePath`, `Unauthorized`,
  `Unavailable`, `Corrupt`, `Io`.

## Ports

- `ObjectStore` — synchronous `put`, `get`, `head`, `list(prefix)` and
  `delete`; implementations own conditional-write mechanics.
  `LocalObjectStore`, `GcsObjectStore` and `S3ObjectStore` implement it here;
  sift implements it for an ephemeral file store.

## Invariants

- Every adapter validates keys: surrounding `/` is trimmed, and an empty key,
  a NUL, a backslash, an empty component, `.` or `..` is `InvalidKey`.
- A put whose condition does not hold fails with `PreconditionFailed`. GCS
  maps `IfAbsent` and `IfVersion` to `ifGenerationMatch`, and a non-numeric
  version fails at once; S3 maps them to `If-None-Match: *` and `If-Match`.
- `LocalObjectStore::open` rejects a root that is a symlink or not a
  directory, rejects symlinks inside object paths, and serializes operations
  with a lock file. It writes content with `atomic_write` under
  `FsyncPolicy::Always`, keeps the content type in a sidecar file, lists keys
  in sorted order, and treats deleting a missing key as success.
- Cloud credentials come from the ambient environment only: an ADC access
  token or the GKE metadata server for GCS, never a key file; the AWS
  credential chain for S3. `STORAGE_EMULATOR_HOST` switches GCS to an
  anonymous emulator.

## Published language

The whole public API; whole-src infrastructure contexts have no application
layer. Every export is at the crate root; the adapter modules are private, so
no old module path needs a facade.

## Exceptions and debts

- **Checker exceptions:** None. Network, file-system and environment
  access are the job of an infrastructure crate.
- **Debts:** none tracked. P2 made the `ObjectMeta` and `Object` fields
  private (D2).
