use std::collections::BTreeMap;

use super::PagedCatalog;
use crate::domain::{
    CatalogEntry, CatalogMutation, CatalogPageBody, CatalogRoot, Result, SegmentError,
    CATALOG_FORMAT_VERSION,
};

impl PagedCatalog {
    pub fn build(
        &self,
        entries: impl IntoIterator<Item = CatalogEntry>,
    ) -> Result<CatalogMutation> {
        let mut sorted = BTreeMap::new();
        for entry in entries {
            self.validate_entry_key(&entry.key)?;
            let key = entry.key.clone();
            if sorted.insert(key.clone(), entry).is_some() {
                return Err(SegmentError::DuplicateCatalogKey { key });
            }
        }
        let entry_count = sorted.len() as u64;
        let leaves = self.pack_leaves(sorted.into_values().collect())?;
        let mut written = Vec::new();
        let mut level = leaves
            .into_iter()
            .map(|entries| {
                self.store_page(CatalogPageBody::Leaf { entries })
                    .inspect(|reference| {
                        written.push(reference.key.clone());
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut height = 0_u16;
        while level.len() > 1 {
            height = height
                .checked_add(1)
                .ok_or_else(|| SegmentError::CorruptCatalog {
                    message: "catalog height exhausted u16".to_string(),
                })?;
            let groups = self.pack_children(level)?;
            level = groups
                .into_iter()
                .map(|children| {
                    self.store_page(CatalogPageBody::Branch { children })
                        .inspect(|reference| {
                            written.push(reference.key.clone());
                        })
                })
                .collect::<Result<Vec<_>>>()?;
        }
        let root = CatalogRoot {
            format_version: CATALOG_FORMAT_VERSION,
            height,
            entry_count,
            page_bytes_limit: self.page_bytes_limit as u32,
            root: level.pop().expect("catalog build always creates one leaf"),
        };
        self.validate_root(&root)?;
        Ok(CatalogMutation {
            root,
            written_page_keys: written,
            obsolete_page_keys: Vec::new(),
        })
    }
}
