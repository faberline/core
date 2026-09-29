use std::collections::VecDeque;

use super::PagedCatalog;
use crate::domain::{
    validate_catalog_key, CatalogEntry, CatalogPageBody, CatalogPageRef, CatalogRoot, Result,
};

impl PagedCatalog {
    pub fn reader(&self, root: &CatalogRoot) -> Result<CatalogReader> {
        self.validate_root(root)?;
        Ok(CatalogReader {
            catalog: self.clone(),
            pending: vec![root.root.clone()],
            leaf: VecDeque::new(),
            after_key: None,
            failed: false,
            peak_buffer_bytes: catalog_ref_bytes(&root.root),
        })
    }

    /// Stream entries strictly after `key` without reading earlier leaf pages.
    pub fn reader_after(&self, root: &CatalogRoot, key: &str) -> Result<CatalogReader> {
        self.validate_root(root)?;
        validate_catalog_key(key)?;
        Ok(CatalogReader {
            catalog: self.clone(),
            pending: vec![root.root.clone()],
            leaf: VecDeque::new(),
            after_key: Some(key.to_string()),
            failed: false,
            peak_buffer_bytes: catalog_ref_bytes(&root.root),
        })
    }

    pub fn page_keys(&self, root: &CatalogRoot) -> Result<CatalogPageKeyReader> {
        self.validate_root(root)?;
        Ok(CatalogPageKeyReader {
            catalog: self.clone(),
            pending: vec![root.root.clone()],
            failed: false,
        })
    }
}

pub struct CatalogReader {
    catalog: PagedCatalog,
    pending: Vec<CatalogPageRef>,
    leaf: VecDeque<CatalogEntry>,
    after_key: Option<String>,
    failed: bool,
    peak_buffer_bytes: usize,
}

pub struct CatalogPageKeyReader {
    catalog: PagedCatalog,
    pending: Vec<CatalogPageRef>,
    failed: bool,
}

impl Iterator for CatalogPageKeyReader {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let reference = self.pending.pop()?;
        match self.catalog.load_page(&reference) {
            Ok(CatalogPageBody::Leaf { .. }) => Some(Ok(reference.key)),
            Ok(CatalogPageBody::Branch { children }) => {
                self.pending.extend(children.into_iter().rev());
                Some(Ok(reference.key))
            }
            Err(error) => {
                self.failed = true;
                Some(Err(error))
            }
        }
    }
}

impl CatalogReader {
    pub fn peak_buffer_bytes(&self) -> usize {
        self.peak_buffer_bytes
    }

    fn measure_buffer(&mut self, loaded_page_bytes: usize) {
        let pending = self.pending.iter().map(catalog_ref_bytes).sum::<usize>();
        let leaf = self
            .leaf
            .iter()
            .map(|entry| entry.key.len() + entry.value.len())
            .sum::<usize>();
        self.peak_buffer_bytes = self
            .peak_buffer_bytes
            .max(loaded_page_bytes.max(pending.saturating_add(leaf)));
    }
}

impl Iterator for CatalogReader {
    type Item = Result<CatalogEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        loop {
            if let Some(entry) = self.leaf.pop_front() {
                self.measure_buffer(0);
                return Some(Ok(entry));
            }
            let reference = self.pending.pop()?;
            if self
                .after_key
                .as_ref()
                .is_some_and(|after| reference.last_key <= *after)
            {
                continue;
            }
            let loaded_page_bytes = reference.bytes as usize;
            match self.catalog.load_page(&reference) {
                Ok(CatalogPageBody::Leaf { mut entries }) => {
                    if let Some(after) = &self.after_key {
                        let first = entries.partition_point(|entry| entry.key <= *after);
                        entries.drain(..first);
                    }
                    self.leaf = entries.into();
                }
                Ok(CatalogPageBody::Branch { children }) => {
                    self.pending.extend(
                        children
                            .into_iter()
                            .filter(|child| {
                                self.after_key
                                    .as_ref()
                                    .is_none_or(|after| child.last_key > *after)
                            })
                            .rev(),
                    );
                }
                Err(error) => {
                    self.failed = true;
                    return Some(Err(error));
                }
            }
            self.measure_buffer(loaded_page_bytes);
        }
    }
}

fn catalog_ref_bytes(reference: &CatalogPageRef) -> usize {
    std::mem::size_of::<CatalogPageRef>()
        + reference.key.len()
        + reference.sha256.len()
        + reference.first_key.len()
        + reference.last_key.len()
}
