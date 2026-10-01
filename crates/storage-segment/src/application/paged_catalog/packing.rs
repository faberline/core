use super::PagedCatalog;
use crate::domain::{
    encode_page, validate_catalog_key, CatalogEntry, CatalogPageBody, CatalogPageRef, Result,
    SegmentError,
};

impl PagedCatalog {
    pub(super) fn pack_leaves(&self, entries: Vec<CatalogEntry>) -> Result<Vec<Vec<CatalogEntry>>> {
        if entries.is_empty() {
            return Ok(vec![Vec::new()]);
        }
        let mut groups = Vec::new();
        let mut current = Vec::new();
        let empty_size = self.page_size(&CatalogPageBody::Leaf {
            entries: Vec::new(),
        })?;
        let mut current_size = empty_size;
        for entry in entries {
            let encoded =
                serde_json::to_vec(&entry).map_err(|error| SegmentError::Serialization {
                    message: error.to_string(),
                })?;
            let added = encoded.len() + usize::from(!current.is_empty());
            if !current.is_empty() && current_size.saturating_add(added) > self.page_bytes_limit {
                groups.push(std::mem::take(&mut current));
                current_size = empty_size;
            }
            let added = encoded.len() + usize::from(!current.is_empty());
            if current_size.saturating_add(added) > self.page_bytes_limit {
                return Err(SegmentError::CatalogPageTooLarge {
                    limit: self.page_bytes_limit,
                });
            }
            current_size = current_size.saturating_add(added);
            current.push(entry);
        }
        if !current.is_empty() {
            groups.push(current);
        }
        Ok(groups)
    }

    pub(super) fn pack_children(
        &self,
        children: Vec<CatalogPageRef>,
    ) -> Result<Vec<Vec<CatalogPageRef>>> {
        if children.is_empty() {
            return Err(SegmentError::CorruptCatalog {
                message: "branch has no children".to_string(),
            });
        }
        let mut groups = Vec::new();
        let mut current = Vec::new();
        let empty_size = self.page_size(&CatalogPageBody::Branch {
            children: Vec::new(),
        })?;
        let mut current_size = empty_size;
        for child in children {
            let encoded =
                serde_json::to_vec(&child).map_err(|error| SegmentError::Serialization {
                    message: error.to_string(),
                })?;
            let added = encoded.len() + usize::from(!current.is_empty());
            if !current.is_empty() && current_size.saturating_add(added) > self.page_bytes_limit {
                groups.push(std::mem::take(&mut current));
                current_size = empty_size;
            }
            let added = encoded.len() + usize::from(!current.is_empty());
            if current_size.saturating_add(added) > self.page_bytes_limit {
                return Err(SegmentError::CatalogPageTooLarge {
                    limit: self.page_bytes_limit,
                });
            }
            current_size = current_size.saturating_add(added);
            current.push(child);
        }
        if !current.is_empty() {
            groups.push(current);
        }
        if groups.len() > 1 && groups.last().is_some_and(|group| group.len() == 1) {
            let only = groups
                .pop()
                .expect("a trailing singleton child group exists")
                .pop()
                .expect("the trailing child group has one item");
            let previous = groups
                .last_mut()
                .expect("a trailing singleton has a previous child group");
            if previous.len() < 3 {
                return Err(SegmentError::CatalogPageTooLarge {
                    limit: self.page_bytes_limit,
                });
            }
            let moved = previous
                .pop()
                .expect("the previous child group has at least three items");
            groups.push(vec![moved, only]);
        }
        Ok(groups)
    }

    pub(super) fn page_size(&self, body: &CatalogPageBody) -> Result<usize> {
        encode_page(body).map(|bytes| bytes.len())
    }

    pub(super) fn validate_entry_key(&self, key: &str) -> Result<()> {
        validate_catalog_key(key)?;
        let reference = CatalogPageRef {
            key: format!("{}/pages/{}.json", self.prefix, "0".repeat(64)),
            sha256: "0".repeat(64),
            bytes: u64::MAX,
            entry_count: u64::MAX,
            first_key: key.to_string(),
            last_key: key.to_string(),
        };
        if self.page_size(&CatalogPageBody::Branch {
            children: vec![reference.clone(), reference.clone(), reference],
        })? > self.page_bytes_limit
        {
            return Err(SegmentError::CatalogPageTooLarge {
                limit: self.page_bytes_limit,
            });
        }
        Ok(())
    }
}
