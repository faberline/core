use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchOperator {
    All,
    Any,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TextQuery {
    All,
    Match {
        field: String,
        text: String,
        operator: MatchOperator,
    },
    Exact {
        field: String,
        value: String,
    },
    And {
        queries: Vec<TextQuery>,
    },
    Or {
        queries: Vec<TextQuery>,
    },
    Not {
        query: Box<TextQuery>,
    },
}

impl TextQuery {
    pub fn match_text(
        field: impl Into<String>,
        text: impl Into<String>,
        operator: MatchOperator,
    ) -> Self {
        Self::Match {
            field: field.into(),
            text: text.into(),
            operator,
        }
    }

    pub fn exact(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Exact {
            field: field.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TextHit {
    pub external_id: String,
    pub version: u64,
    pub score: f32,
}
