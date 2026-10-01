use super::page::{CatalogPageBody, CatalogPageRef};
use crate::domain::{Result, SegmentError};

pub(crate) fn page_bounds(body: &CatalogPageBody) -> Result<(u64, String, String)> {
    match body {
        CatalogPageBody::Leaf { entries } => Ok((
            entries.len() as u64,
            entries
                .first()
                .map(|entry| entry.key().to_string())
                .unwrap_or_default(),
            entries
                .last()
                .map(|entry| entry.key().to_string())
                .unwrap_or_default(),
        )),
        CatalogPageBody::Branch { children } => Ok((
            children.iter().try_fold(0_u64, |count, child| {
                count
                    .checked_add(child.entry_count)
                    .ok_or_else(|| SegmentError::CorruptCatalog {
                        message: "catalog entry count exhausted u64".to_string(),
                    })
            })?,
            children
                .first()
                .expect("validated branch")
                .first_key
                .clone(),
            children.last().expect("validated branch").last_key.clone(),
        )),
    }
}

pub(crate) fn child_index(children: &[CatalogPageRef], key: &str) -> Result<usize> {
    if children.is_empty() {
        return Err(SegmentError::CorruptCatalog {
            message: "branch has no children".to_string(),
        });
    }
    Ok(children
        .partition_point(|child| child.last_key.as_str() < key)
        .min(children.len() - 1))
}
