use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;

use super::document::TextDocument;
use super::error::{IndexError, Result};
use super::query::{MatchOperator, TextHit, TextQuery};
use super::schema::{FieldKind, TextSchema};
use super::snapshot::{TextIndexSnapshot, SNAPSHOT_FORMAT_VERSION};
use super::text_index::TextIndex;
use super::tokenize::tokenize;
use super::{DocumentId, DocumentVersion};

/// Deterministic in-process index. Durability comes from its typed snapshot or
/// from rebuilding it from the product's committed segments.
pub struct MemoryTextIndex {
    schema: TextSchema,
    state: RwLock<MemoryTextState>,
}

#[derive(Default)]
struct MemoryTextState {
    documents: BTreeMap<DocumentId, TextDocument>,
    tombstones: BTreeMap<DocumentId, DocumentVersion>,
}

impl MemoryTextIndex {
    pub fn new(schema: TextSchema) -> Result<Self> {
        // Re-run validation for deserialized schemas.
        let schema = TextSchema::new(schema.fields)?;
        Ok(Self {
            schema,
            state: RwLock::new(MemoryTextState::default()),
        })
    }

    fn validate_external_id(&self, external_id: &DocumentId) -> Result<()> {
        if external_id.as_str().trim().is_empty() || external_id.as_str().contains('\0') {
            return Err(IndexError::InvalidDocument {
                message: "external_id must not be empty or contain NUL".to_string(),
            });
        }
        Ok(())
    }

    fn validate_document(&self, document: &TextDocument) -> Result<()> {
        self.validate_external_id(document.external_id())?;
        for field in document.fields().keys() {
            self.schema.field(field)?;
        }
        Ok(())
    }

    fn build_state(
        &self,
        documents: Vec<TextDocument>,
        tombstones: BTreeMap<DocumentId, DocumentVersion>,
    ) -> Result<MemoryTextState> {
        let mut rebuilt = BTreeMap::<DocumentId, TextDocument>::new();
        for document in documents {
            self.validate_document(&document)?;
            if rebuilt
                .get(document.external_id())
                .is_none_or(|current| current.version() < document.version())
            {
                rebuilt.insert(document.external_id().clone(), document);
            }
        }

        let mut retained_tombstones = BTreeMap::new();
        for (external_id, delete_version) in tombstones {
            self.validate_external_id(&external_id)?;
            if rebuilt
                .get(&external_id)
                .is_some_and(|document| document.version() > delete_version)
            {
                continue;
            }
            rebuilt.remove(&external_id);
            retained_tombstones.insert(external_id, delete_version);
        }
        Ok(MemoryTextState {
            documents: rebuilt,
            tombstones: retained_tombstones,
        })
    }

    fn validate_query(&self, query: &TextQuery) -> Result<()> {
        match query {
            TextQuery::All => Ok(()),
            TextQuery::Match { field, .. } => match self.schema.field(field)?.kind {
                FieldKind::Text { .. } => Ok(()),
                FieldKind::Keyword => Err(IndexError::UnsupportedFieldOperation {
                    field: field.clone(),
                    operation: "text match",
                }),
            },
            TextQuery::Exact { field, .. } => match self.schema.field(field)?.kind {
                FieldKind::Keyword => Ok(()),
                FieldKind::Text { .. } => Err(IndexError::UnsupportedFieldOperation {
                    field: field.clone(),
                    operation: "exact match",
                }),
            },
            TextQuery::And { queries } | TextQuery::Or { queries } => {
                for query in queries {
                    self.validate_query(query)?;
                }
                Ok(())
            }
            TextQuery::Not { query } => self.validate_query(query),
        }
    }

    fn evaluate(&self, document: &TextDocument, query: &TextQuery) -> Option<f32> {
        match query {
            TextQuery::All => Some(1.0),
            TextQuery::Match {
                field,
                text,
                operator,
            } => {
                let FieldKind::Text { analyzer } = self.schema.fields.get(field)?.kind else {
                    return None;
                };
                let query_terms = tokenize(text, analyzer);
                if query_terms.is_empty() {
                    return None;
                }
                let document_terms = document
                    .fields()
                    .get(field)
                    .map(|value| tokenize(value, analyzer))
                    .unwrap_or_default()
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                let matched = query_terms
                    .iter()
                    .filter(|term| document_terms.contains(*term))
                    .count();
                let accepts = match operator {
                    MatchOperator::All => matched == query_terms.len(),
                    MatchOperator::Any => matched > 0,
                };
                accepts.then_some(matched as f32 / query_terms.len() as f32)
            }
            TextQuery::Exact { field, value } => document
                .fields()
                .get(field)
                .is_some_and(|actual| actual == value)
                .then_some(1.0),
            TextQuery::And { queries } => {
                let mut score = 0.0;
                for query in queries {
                    score += self.evaluate(document, query)?;
                }
                Some(score.max(1.0))
            }
            TextQuery::Or { queries } => queries
                .iter()
                .filter_map(|query| self.evaluate(document, query))
                .reduce(f32::max),
            TextQuery::Not { query } => self.evaluate(document, query).is_none().then_some(1.0),
        }
    }
}

impl TextIndex for MemoryTextIndex {
    fn schema(&self) -> TextSchema {
        self.schema.clone()
    }

    fn upsert(&self, document: TextDocument) -> Result<()> {
        self.validate_document(&document)?;
        let mut state = self.state.write().map_err(|_| IndexError::LockPoisoned)?;
        if state
            .tombstones
            .get(document.external_id())
            .is_some_and(|delete_version| *delete_version >= document.version())
            || state
                .documents
                .get(document.external_id())
                .is_some_and(|current| current.version() >= document.version())
        {
            return Ok(());
        }
        state.tombstones.remove(document.external_id());
        state
            .documents
            .insert(document.external_id().clone(), document);
        Ok(())
    }

    fn delete(&self, external_id: &DocumentId, version: Option<DocumentVersion>) -> Result<bool> {
        self.validate_external_id(external_id)?;
        let mut state = self.state.write().map_err(|_| IndexError::LockPoisoned)?;
        let current_version = state
            .documents
            .get(external_id)
            .map(|document| document.version());
        let delete_version = match (version, current_version) {
            (Some(delete_version), Some(current_version)) if current_version > delete_version => {
                return Ok(false);
            }
            (Some(delete_version), _) => delete_version,
            (None, Some(current_version)) => current_version,
            (None, None) => return Ok(false),
        };
        let removed = state.documents.remove(external_id).is_some();
        state
            .tombstones
            .entry(external_id.clone())
            .and_modify(|current| *current = (*current).max(delete_version))
            .or_insert(delete_version);
        Ok(removed)
    }

    fn search(&self, query: &TextQuery, limit: usize) -> Result<Vec<TextHit>> {
        self.validate_query(query)?;
        if limit == 0 {
            return Ok(Vec::new());
        }
        let state = self.state.read().map_err(|_| IndexError::LockPoisoned)?;
        let mut hits = state
            .documents
            .values()
            .filter_map(|document| {
                self.evaluate(document, query).map(|score| TextHit {
                    external_id: document.external_id().clone(),
                    version: document.version(),
                    score,
                })
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.external_id.cmp(&right.external_id))
        });
        hits.truncate(limit);
        Ok(hits)
    }

    fn snapshot(&self) -> Result<TextIndexSnapshot> {
        let state = self.state.read().map_err(|_| IndexError::LockPoisoned)?;
        Ok(TextIndexSnapshot {
            format_version: SNAPSHOT_FORMAT_VERSION,
            schema: self.schema.clone(),
            documents: state.documents.values().cloned().collect(),
            tombstones: state.tombstones.clone(),
        })
    }

    fn restore(&self, snapshot: &TextIndexSnapshot) -> Result<()> {
        if snapshot.format_version != SNAPSHOT_FORMAT_VERSION {
            return Err(IndexError::CorruptSnapshot {
                message: format!(
                    "unsupported format version {}; expected {SNAPSHOT_FORMAT_VERSION}",
                    snapshot.format_version
                ),
            });
        }
        if snapshot.schema != self.schema {
            return Err(IndexError::CorruptSnapshot {
                message: "snapshot schema does not match the opened index".to_string(),
            });
        }
        let restored = self.build_state(snapshot.documents.clone(), snapshot.tombstones.clone())?;
        *self.state.write().map_err(|_| IndexError::LockPoisoned)? = restored;
        Ok(())
    }

    fn rebuild(&self, documents: Vec<TextDocument>) -> Result<()> {
        let rebuilt = self.build_state(documents, BTreeMap::new())?;
        *self.state.write().map_err(|_| IndexError::LockPoisoned)? = rebuilt;
        Ok(())
    }
}
