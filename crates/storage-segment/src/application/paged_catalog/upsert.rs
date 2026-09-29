use super::PagedCatalog;
use crate::domain::{
    child_index, CatalogEntry, CatalogMutation, CatalogPageBody, CatalogPageRef, CatalogRoot,
    Result, SegmentError, CATALOG_FORMAT_VERSION,
};

impl PagedCatalog {
    pub fn upsert(&self, root: &CatalogRoot, entry: CatalogEntry) -> Result<CatalogMutation> {
        self.validate_root(root)?;
        self.validate_entry_key(&entry.key)?;
        let inserted = self.insert_page(&root.root, entry)?;
        let (root_ref, height) = if inserted.pages.len() == 1 {
            (inserted.pages[0].clone(), root.height)
        } else {
            let reference = self.store_page(CatalogPageBody::Branch {
                children: inserted.pages.clone(),
            })?;
            (
                reference,
                root.height
                    .checked_add(1)
                    .ok_or_else(|| SegmentError::CorruptCatalog {
                        message: "catalog height exhausted u16".to_string(),
                    })?,
            )
        };
        let mut written = inserted.written;
        if inserted.pages.len() > 1 {
            written.push(root_ref.key.clone());
        }
        let next = CatalogRoot {
            format_version: CATALOG_FORMAT_VERSION,
            height,
            entry_count: root
                .entry_count
                .checked_add(u64::from(inserted.inserted))
                .ok_or_else(|| SegmentError::CorruptCatalog {
                    message: "catalog entry count exhausted u64".to_string(),
                })?,
            page_bytes_limit: self.page_bytes_limit as u32,
            root: root_ref,
        };
        self.validate_root(&next)?;
        Ok(CatalogMutation {
            root: next,
            written_page_keys: written,
            obsolete_page_keys: inserted.obsolete,
        })
    }

    fn insert_page(&self, reference: &CatalogPageRef, entry: CatalogEntry) -> Result<InsertPage> {
        match self.load_page(reference)? {
            CatalogPageBody::Leaf { mut entries } => {
                let (inserted, changed) =
                    match entries.binary_search_by(|current| current.key.cmp(&entry.key)) {
                        Ok(index) if entries[index] == entry => (false, false),
                        Ok(index) => {
                            entries[index] = entry;
                            (false, true)
                        }
                        Err(index) => {
                            entries.insert(index, entry);
                            (true, true)
                        }
                    };
                if !changed {
                    return Ok(InsertPage {
                        pages: vec![reference.clone()],
                        inserted,
                        written: Vec::new(),
                        obsolete: Vec::new(),
                    });
                }
                let mut pages = Vec::new();
                let mut written = Vec::new();
                for entries in self.pack_leaves(entries)? {
                    let page = self.store_page(CatalogPageBody::Leaf { entries })?;
                    written.push(page.key.clone());
                    pages.push(page);
                }
                Ok(InsertPage {
                    pages,
                    inserted,
                    written,
                    obsolete: vec![reference.key.clone()],
                })
            }
            CatalogPageBody::Branch { mut children } => {
                let index = child_index(&children, &entry.key)?;
                let child = self.insert_page(&children[index], entry)?;
                if child.written.is_empty() {
                    return Ok(InsertPage {
                        pages: vec![reference.clone()],
                        inserted: child.inserted,
                        written: Vec::new(),
                        obsolete: child.obsolete,
                    });
                }
                children.splice(index..=index, child.pages);
                let mut pages = Vec::new();
                let mut written = child.written;
                for children in self.pack_children(children)? {
                    let page = self.store_page(CatalogPageBody::Branch { children })?;
                    written.push(page.key.clone());
                    pages.push(page);
                }
                let mut obsolete = child.obsolete;
                obsolete.push(reference.key.clone());
                Ok(InsertPage {
                    pages,
                    inserted: child.inserted,
                    written,
                    obsolete,
                })
            }
        }
    }
}

struct InsertPage {
    pages: Vec<CatalogPageRef>,
    inserted: bool,
    written: Vec<String>,
    obsolete: Vec<String>,
}
