# Release notes 0.5.0: storage crates

Range checked: `v0.4.14` (`029e765e`) to `origin/main` (`627d8121`).

## Crates checked

- `crates/storage-durable`
- `crates/storage-segment`
- `crates/storage-object`
- `crates/index-text`

## Breaking changes

### storage-durable: `DataRoot` and `DataRootPolicy` return `DataRootError`

Before: `DataRoot::open`, `DataRoot::replace_manifest`, `DataRootPolicy::create_manifest` and `DataRootPolicy::validate_manifest` returned `anyhow::Result`.
After: `DataRoot::open -> Result<Self, DataRootError>`, `replace_manifest -> Result<(), DataRootError>`, `create_manifest -> Result<Self::Manifest, DataRootError>`, `validate_manifest -> Result<(), DataRootError>`. `DataRootError` is exported at the crate root. `reject_symlink`, `set_private_file_mode` and `set_private_directory_mode` still return `anyhow::Result<()>`.
Commit: `f6b54810`.

Migration: callers that use `?` into `anyhow` need no change. A policy must return `DataRootError::other(err)` for its own failures. Code that matches on the error must match `DataRootError` variants.

### storage-durable: `LogFrame` fields are private

Before: `pub seq: u64` and `pub payload: Vec<u8>`.
After: `LogFrame::new(seq, payload)`, `seq() -> u64`, `payload() -> &[u8]`, `into_payload(self) -> Vec<u8>`.
Commit: `7ea0fd1f`.

Migration: `frame.seq` becomes `frame.seq()`, `frame.payload` becomes `frame.payload()`, and a moved payload becomes `frame.into_payload()`. Build with `LogFrame::new(seq, payload)`. `MappedLogFrame.seq` stays public.

### storage-durable: four public methods are deleted

Removed: `FramedLogWriter::finish_sync`, `FramedLogCursor::reread_frame_at`, `FramedLogCursor::reread_mapped_frame_at` and `GenerationStore::commit_from_current`.
Commit: `325e3367`.

Migration:
- `finish_sync(plan)`: call `writer.begin_sync()`. When it returns `Some(plan)`, call `plan.sync_off_lock()` with the writer lock released, then `writer.complete_sync(plan)`. `None` means no sync is due.
- `reread_frame_at` (owned `LogFrame`): there is no replacement. Use `reread_large_mapped_frame_at(offset)`, which returns a `MappedLogFrame`, or copy the payload out of it.
- `reread_mapped_frame_at`: use `reread_large_mapped_frame_at(offset)`. It covers large frames only, and no reread is left for other mapped frames.
- `commit_from_current(staged)`: call `commit_from_current_with_publication_guard(staged, || Ok(()))`.

### storage-object: `ObjectMeta` and `Object` fields are private

Before: `ObjectMeta` had public fields `key`, `size`, `content_type`, `version`, `etag` and `updated`. `Object` had public fields `meta` and `bytes`.
After: `ObjectMeta::new(key, size, content_type, version)`, then `with_etag(Option<String>)` and `with_updated(Option<String>)`. Accessors: `key()`, `size()`, `content_type()`, `version() -> &ObjectVersion`, `etag() -> Option<&str>`, `updated() -> Option<&str>`. `Object::new(meta, bytes)`, `meta()`, `bytes()` and `into_parts()`.
Commit: `c91d4880`.

Migration: replace field reads with the accessors. Replace struct literals with `ObjectMeta::new(..)` plus the `with_` methods, and `Object { meta, bytes }` with `Object::new(meta, bytes)`. The JSON form is unchanged: the same field names and derives, pinned by commit `5d5784b6`.

### storage-segment: the `SegmentStore` trait is deleted

Before: `pub trait SegmentStore<Record>` with `type Descriptor`, `write_immutable` and `read`.
After: removed. The trait had no implementation in this repository, and no item replaces it.
Commit: `11dc57a7`.

Migration: delete any `impl SegmentStore` and any bound on it. An app that needs this behavior owns it.

### storage-segment: `CatalogEntry` fields are private and checked

Before: `CatalogEntry { pub key, pub value }`, built as a struct literal with no check.
After: `CatalogEntry::try_new(key, value) -> Result<Self>`, which returns `InvalidCatalogKey` for an empty key, a key over 1024 bytes or a key with a NUL. Accessors: `key()`, `value()` and `into_parts()`.
Commit: `57784f4a`.

Migration: replace the literal with `CatalogEntry::try_new(key, value)?`. Read with `key()` and `value()`. An app that built entries with an invalid key now gets an error at construction.

### storage-segment: `ArchivedObject.version` is `ArchivedObjectVersion`

Before: `version: storage_object::ObjectVersion`.
After: `version: ArchivedObjectVersion`, a domain-owned newtype with `new(impl Into<String>)` and `as_str()`. It serializes as a bare string, as before.
Commit: `38edfa99`.

Migration: build with `ArchivedObjectVersion::new(s)` and read with `.as_str()`. Convert an `ObjectVersion` with `ArchivedObjectVersion::new(v.as_str())`.

### storage-segment: `SegmentError::ObjectStore` holds a boxed error

Before: `ObjectStore(#[from] ObjectStoreError)`.
After: `ObjectStore(Box<dyn std::error::Error + Send + Sync>)`. `From<ObjectStoreError>` is still implemented, so `?` still works.
Commit: `383f720c`.

Migration: a `match` that binds `ObjectStoreError` directly must use `e.downcast_ref::<ObjectStoreError>()` on the boxed error.

### index-text: `TextDocument` fields are private and typed

Before: `TextDocument { pub external_id: String, pub version: u64, pub fields }`, and `TextDocument::new(impl Into<String>, u64)`.
After: `TextDocument::new(DocumentId, DocumentVersion)`, with `with_field`. Accessors: `external_id() -> &DocumentId`, `version() -> DocumentVersion` and `fields() -> &BTreeMap<String, String>`.
Commit: `3b12729a`, with `430d6e4e`.

Migration: `TextDocument::new(DocumentId::new("doc"), DocumentVersion::new(1))`. Read with the accessors.

### index-text: `TextHit` ids and versions are typed

Before: `TextHit { pub external_id: String, pub version: u64, pub score }`.
After: `external_id: DocumentId` and `version: DocumentVersion`. `score` is unchanged.
Commit: `430d6e4e`.

Migration: use `hit.external_id.as_str()` and `hit.version.get()`.

### index-text: `TextIndex::delete` takes typed ids

Before: `delete(&self, external_id: &str, version: Option<u64>) -> Result<bool>`.
After: `delete(&self, external_id: &DocumentId, version: Option<DocumentVersion>) -> Result<bool>`.
Commit: `430d6e4e`.

Migration: wrap the arguments with `DocumentId::new(..)` and `DocumentVersion::new(..)`.

### index-text: `TextIndexSnapshot.tombstones` is typed

Before: `BTreeMap<String, u64>`.
After: `BTreeMap<DocumentId, DocumentVersion>`. The JSON is unchanged, pinned by commit `ebb26472`.
Commit: `430d6e4e`.

Migration: key the map with `DocumentId` and use `DocumentVersion` values.

## Fixes apps must take

None. The range has no fix commits for these crates. Its commits are refactors, breaking API changes, module moves and tests.

## New

- storage-durable: `DataRootError` and `DataRootError::other`, the typed error of `DataRoot` and its policy.
- storage-durable: `LogFrame::new`, `seq()`, `payload()` and `into_payload()`.
- storage-segment: `ArchivedObjectVersion`, the domain-owned object version, with `new` and `as_str`.
- storage-segment: `CatalogEntry::try_new`, `key()`, `value()` and `into_parts()`, the checked constructor and accessors.
- storage-object: `ObjectMeta::new`, `with_etag`, `with_updated` and accessors, the constructor and accessors for the metadata.
- storage-object: `Object::new`, `meta()`, `bytes()` and `into_parts()`, the constructor and accessors for an object.
- index-text: `DocumentId`, the document key newtype, with `new` and `as_str`.
- index-text: `DocumentVersion`, the version newtype, with `new` and `get`.
- index-text: `TextDocument::external_id()`, `version()` and `fields()`, the accessors that replace the public fields.
