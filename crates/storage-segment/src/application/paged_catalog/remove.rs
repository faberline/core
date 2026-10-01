use super::PagedCatalog;
use crate::domain::{
    child_index, validate_catalog_key, CatalogMutation, CatalogPageBody, CatalogPageRef,
    CatalogRoot, Result, SegmentError, CATALOG_FORMAT_VERSION,
};

impl PagedCatalog {
    /// Remove one key with copy-on-write updates along only its search path.
    /// A missing key returns the original root without writing pages.
    pub fn remove(&self, root: &CatalogRoot, key: &str) -> Result<CatalogMutation> {
        self.validate_root(root)?;
        validate_catalog_key(key)?;
        let removed = self.remove_page(&root.root, key)?;
        if !removed.removed {
            return Ok(CatalogMutation {
                root: root.clone(),
                written_page_keys: Vec::new(),
                obsolete_page_keys: Vec::new(),
            });
        }
        let root_ref = if removed.pages.is_empty() {
            self.store_page(CatalogPageBody::Leaf {
                entries: Vec::new(),
            })?
        } else if removed.pages.len() == 1 {
            removed.pages[0].clone()
        } else {
            self.store_page(CatalogPageBody::Branch {
                children: removed.pages.clone(),
            })?
        };
        let mut written = removed.written;
        if removed.pages.len() != 1 {
            written.push(root_ref.key.clone());
        }
        let next = CatalogRoot {
            format_version: CATALOG_FORMAT_VERSION,
            height: if removed.pages.is_empty() {
                0
            } else if removed.pages.len() > 1 {
                root.height
                    .checked_add(1)
                    .ok_or_else(|| SegmentError::CorruptCatalog {
                        message: "catalog height exhausted u16".to_string(),
                    })?
            } else {
                root.height
            },
            entry_count: root.entry_count.checked_sub(1).ok_or_else(|| {
                SegmentError::CorruptCatalog {
                    message: "catalog entry count underflow".to_string(),
                }
            })?,
            page_bytes_limit: self.page_bytes_limit as u32,
            root: root_ref,
        };
        self.validate_root(&next)?;
        Ok(CatalogMutation {
            root: next,
            written_page_keys: written,
            obsolete_page_keys: removed.obsolete,
        })
    }

    fn remove_page(&self, reference: &CatalogPageRef, key: &str) -> Result<RemovePage> {
        match self.load_page(reference)? {
            CatalogPageBody::Leaf { mut entries } => {
                let Ok(index) = entries.binary_search_by(|entry| entry.key.as_str().cmp(key))
                else {
                    return Ok(RemovePage {
                        pages: vec![reference.clone()],
                        removed: false,
                        written: Vec::new(),
                        obsolete: Vec::new(),
                    });
                };
                entries.remove(index);
                if entries.is_empty() {
                    return Ok(RemovePage {
                        pages: Vec::new(),
                        removed: true,
                        written: Vec::new(),
                        obsolete: vec![reference.key.clone()],
                    });
                }
                let mut pages = Vec::new();
                let mut written = Vec::new();
                for entries in self.pack_leaves(entries)? {
                    let page = self.store_page(CatalogPageBody::Leaf { entries })?;
                    written.push(page.key.clone());
                    pages.push(page);
                }
                Ok(RemovePage {
                    pages,
                    removed: true,
                    written,
                    obsolete: vec![reference.key.clone()],
                })
            }
            CatalogPageBody::Branch { mut children } => {
                let index = child_index(&children, key)?;
                if key < children[index].first_key.as_str()
                    || key > children[index].last_key.as_str()
                {
                    return Ok(RemovePage {
                        pages: vec![reference.clone()],
                        removed: false,
                        written: Vec::new(),
                        obsolete: Vec::new(),
                    });
                }
                let child = self.remove_page(&children[index], key)?;
                if !child.removed {
                    return Ok(RemovePage {
                        pages: vec![reference.clone()],
                        removed: false,
                        written: Vec::new(),
                        obsolete: child.obsolete,
                    });
                }
                children.splice(index..=index, child.pages);
                if children.is_empty() {
                    let mut obsolete = child.obsolete;
                    obsolete.push(reference.key.clone());
                    return Ok(RemovePage {
                        pages: Vec::new(),
                        removed: true,
                        written: child.written,
                        obsolete,
                    });
                }
                let mut pages = Vec::new();
                let mut written = child.written;
                for children in self.pack_children(children)? {
                    let page = self.store_page(CatalogPageBody::Branch { children })?;
                    written.push(page.key.clone());
                    pages.push(page);
                }
                let mut obsolete = child.obsolete;
                obsolete.push(reference.key.clone());
                Ok(RemovePage {
                    pages,
                    removed: true,
                    written,
                    obsolete,
                })
            }
        }
    }
}

struct RemovePage {
    pages: Vec<CatalogPageRef>,
    removed: bool,
    written: Vec<String>,
    obsolete: Vec<String>,
}
