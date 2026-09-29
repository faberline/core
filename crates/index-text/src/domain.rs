//! The text index: schema, documents, queries, snapshots, the in-process
//! index, and the analyzers that tokenize field text.

mod document;
mod error;
mod memory_text_index;
mod query;
mod schema;
mod snapshot;
mod text_index;
mod tokenize;

pub use document::TextDocument;
pub use error::{IndexError, Result};
pub use memory_text_index::MemoryTextIndex;
pub use query::{MatchOperator, TextHit, TextQuery};
pub use schema::{Analyzer, FieldKind, FieldSpec, TextSchema};
pub use snapshot::{TextIndexSnapshot, SNAPSHOT_FORMAT_VERSION};
pub use text_index::TextIndex;
#[cfg(feature = "jieba")]
pub use tokenize::{for_jieba_no_hmm, JiebaRouteStore};
pub use tokenize::{
    for_whitespace_lower, for_whitespace_lower_cow, tokenize, DEFAULT_NGRAM_MAX, DEFAULT_NGRAM_MIN,
};
