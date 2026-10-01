//! Shared, rebuildable text-index contract.
//!
//! Products own their records and query language. This crate owns schema
//! validation, text analysis, version ordering, snapshots, and rebuilds.

mod domain;

#[cfg(feature = "jieba")]
pub use domain::{for_jieba_no_hmm, JiebaRouteStore};
pub use domain::{
    for_whitespace_lower, for_whitespace_lower_cow, tokenize, Analyzer, FieldKind, FieldSpec,
    IndexError, MatchOperator, MemoryTextIndex, Result, TextDocument, TextHit, TextIndex,
    TextIndexSnapshot, TextQuery, TextSchema, DEFAULT_NGRAM_MAX, DEFAULT_NGRAM_MIN,
    SNAPSHOT_FORMAT_VERSION,
};
