use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::error::{IndexError, Result};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Analyzer {
    WhitespaceLower,
    Jieba,
    Ngram,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldKind {
    Text { analyzer: Analyzer },
    Keyword,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FieldSpec {
    pub kind: FieldKind,
}

impl FieldSpec {
    pub fn text(analyzer: Analyzer) -> Self {
        Self {
            kind: FieldKind::Text { analyzer },
        }
    }

    pub fn keyword() -> Self {
        Self {
            kind: FieldKind::Keyword,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextSchema {
    pub(super) fields: BTreeMap<String, FieldSpec>,
}

impl TextSchema {
    pub fn new(fields: BTreeMap<String, FieldSpec>) -> Result<Self> {
        if fields.is_empty() {
            return Err(IndexError::InvalidSchema {
                message: "text index needs at least one field".to_string(),
            });
        }
        if let Some(name) = fields
            .keys()
            .find(|name| name.trim().is_empty() || name.contains('\0'))
        {
            return Err(IndexError::InvalidSchema {
                message: format!("invalid field name {name:?}"),
            });
        }
        Ok(Self { fields })
    }

    pub fn fields(&self) -> &BTreeMap<String, FieldSpec> {
        &self.fields
    }

    pub(super) fn field(&self, name: &str) -> Result<&FieldSpec> {
        self.fields
            .get(name)
            .ok_or_else(|| IndexError::UnknownField {
                field: name.to_string(),
            })
    }
}
