use serde::{Deserialize, Serialize};

use super::entry::CatalogEntry;
use super::key::validate_catalog_key;
use crate::domain::{Result, SegmentError};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogPageRef {
    pub key: String,
    pub sha256: String,
    pub bytes: u64,
    pub entry_count: u64,
    pub first_key: String,
    pub last_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct CatalogPage {
    pub(crate) format_version: u16,
    #[serde(flatten)]
    pub(crate) body: CatalogPageBody,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum CatalogPageBody {
    Leaf { entries: Vec<CatalogEntry> },
    Branch { children: Vec<CatalogPageRef> },
}

pub(crate) fn validate_page_body(body: &CatalogPageBody) -> Result<()> {
    match body {
        CatalogPageBody::Leaf { entries } => {
            let mut previous: Option<&str> = None;
            for entry in entries {
                validate_catalog_key(entry.key())?;
                if previous.is_some_and(|key| key >= entry.key()) {
                    return Err(SegmentError::CorruptCatalog {
                        message: "leaf keys are not strictly sorted".to_string(),
                    });
                }
                previous = Some(entry.key());
            }
        }
        CatalogPageBody::Branch { children } => {
            if children.is_empty() {
                return Err(SegmentError::CorruptCatalog {
                    message: "branch has no children".to_string(),
                });
            }
            let mut previous: Option<&str> = None;
            for child in children {
                if child.entry_count == 0
                    || child.first_key.is_empty()
                    || child.first_key > child.last_key
                    || previous.is_some_and(|key| key >= child.first_key.as_str())
                {
                    return Err(SegmentError::CorruptCatalog {
                        message: "branch child ranges are invalid".to_string(),
                    });
                }
                previous = Some(&child.last_key);
            }
        }
    }
    Ok(())
}
