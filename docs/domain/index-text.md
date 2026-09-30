# index-text

index-text is a rebuildable full-text index. Products own their records and
their query language; this context owns schema validation, text analysis,
version ordering, snapshots and rebuilds. The index is never the source of
truth: it is restored from its own snapshot or rebuilt from the product's
committed records. No core crate depends on it; downstream, sift keeps an
index with it, and lumen uses its analyzers.

**Form:** domain only · **Depends on:** — · **Crate:** [`crates/index-text`](../../crates/index-text)

## Model

- **Analyzer** — `Analyzer`: `WhitespaceLower`, `Jieba` or `Ngram`. A token
  stream comes from `tokenize` and the streaming `for_whitespace_lower`,
  `for_whitespace_lower_cow` and, with the `jieba` feature, `for_jieba_no_hmm`.
- **Field** — `FieldSpec` with a `FieldKind`: `Text { analyzer }` for
  analyzed search, or `Keyword` for exact match.
- **Schema** — `TextSchema`: the named fields of one index.
- **Document** — `TextDocument`: an external id, a `u64` version and field
  values. The version orders every write to the same id. The fields are
  private: `TextDocument::new(external_id, version)` and `with_field` build
  it, and `external_id()`, `version()` and `fields()` read it.
- **Tombstone** — the highest delete version seen for an absent document.
- **Query** — `TextQuery`: `All`, `Match` (field, text, `MatchOperator` `All`
  or `Any`), `Exact`, `And`, `Or`, `Not`. `TextHit` is an id, version and
  score.
- **Index snapshot** — `TextIndexSnapshot`: the versioned JSON image of an
  index (format version, schema, documents, tombstones).
- **Index error** — `IndexError`: invalid schema or document, unknown field,
  an operation the field does not support, a corrupt snapshot, a poisoned lock.

## Ports

- `TextIndex` — the product-neutral index boundary: schema, upsert, delete,
  search, snapshot, restore, rebuild. `MemoryTextIndex` implements it as a
  deterministic in-process index.

## Invariants

- A schema has at least one field, and field names are non-empty and free of
  NUL. `MemoryTextIndex::new` validates the schema again.
- An upsert with a version not above the current document's or the tombstone's
  is ignored. A delete with a version older than the current document returns
  `false`; tombstones keep the highest delete version.
- `Match` needs a `Text` field and `Exact` needs a `Keyword` field. A match
  scores the fraction of query terms found; with `All`, every term must match.
- Hits sort by score descending, then by external id; a limit of 0 returns no
  hits.
- `restore` requires the current snapshot format version and an equal schema.
  `rebuild` keeps the highest version per id and clears tombstones.
- `SNAPSHOT_FORMAT_VERSION` is 1; a version-1 snapshot without tombstones
  decodes with none.
- Without the `jieba` feature, the `Jieba` analyzer falls back to CJK bigrams.

## Published language

The whole public API; domain-only contexts have no application layer. sift
uses the schema, field, document, query and snapshot types with `TextIndex`
and `MemoryTextIndex`; lumen uses `Analyzer`, `tokenize`,
`for_whitespace_lower_cow`, `for_jieba_no_hmm` and the n-gram defaults.

## Exceptions and debts

- **Checker exceptions (P1):** B2 (`jieba-rs`): the optional `jieba` feature
  exposes `for_jieba_no_hmm` and re-exports `jieba_rs::RouteStore` as
  `JiebaRouteStore`. The dictionary is the analyzer's own vocabulary, so this
  stays with a long-term reason.
- **Tracked for P2:** bare ids: `TextDocument::external_id` is a `String` and
  its version a `u64`.
