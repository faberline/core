use super::document::TextDocument;
use super::error::Result;
use super::query::{TextHit, TextQuery};
use super::schema::TextSchema;
use super::snapshot::TextIndexSnapshot;

/// Product-neutral index boundary. The caller remains the source of truth.
pub trait TextIndex: Send + Sync {
    fn schema(&self) -> TextSchema;
    fn upsert(&self, document: TextDocument) -> Result<()>;
    fn delete(&self, external_id: &str, version: Option<u64>) -> Result<bool>;
    fn search(&self, query: &TextQuery, limit: usize) -> Result<Vec<TextHit>>;
    fn snapshot(&self) -> Result<TextIndexSnapshot>;
    fn restore(&self, snapshot: &TextIndexSnapshot) -> Result<()>;
    fn rebuild(&self, documents: Vec<TextDocument>) -> Result<()>;
}
