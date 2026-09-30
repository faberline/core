use super::PagedCatalog;
use crate::domain::{
    child_index, lexicographic_successor, validate_catalog_key, CatalogEntry, CatalogPageBody,
    CatalogRoot, Result,
};

impl PagedCatalog {
    pub fn lookup(&self, root: &CatalogRoot, key: &str) -> Result<Option<CatalogEntry>> {
        self.validate_root(root)?;
        validate_catalog_key(key)?;
        let mut reference = root.root.clone();
        loop {
            match self.load_page(&reference)? {
                CatalogPageBody::Leaf { entries } => {
                    return Ok(entries
                        .binary_search_by(|entry| entry.key().cmp(key))
                        .ok()
                        .map(|index| entries[index].clone()));
                }
                CatalogPageBody::Branch { children } => {
                    let index = child_index(&children, key)?;
                    reference = children[index].clone();
                }
            }
        }
    }

    /// Return the last entry in one lexicographic prefix with one tree-path
    /// read. Callers can keep exact per-partition high-water marks without a
    /// full catalog scan.
    pub fn last_with_prefix(
        &self,
        root: &CatalogRoot,
        prefix: &str,
    ) -> Result<Option<CatalogEntry>> {
        self.validate_root(root)?;
        validate_catalog_key(prefix)?;
        let candidate = match lexicographic_successor(prefix) {
            Some(upper) => self.last_before(root, &upper)?,
            None => self.last_entry(root)?,
        };
        Ok(candidate.filter(|entry| entry.key().starts_with(prefix)))
    }

    fn last_before(&self, root: &CatalogRoot, upper: &str) -> Result<Option<CatalogEntry>> {
        let mut reference = root.root.clone();
        loop {
            match self.load_page(&reference)? {
                CatalogPageBody::Leaf { entries } => {
                    let index = entries.partition_point(|entry| entry.key() < upper);
                    return Ok(index.checked_sub(1).map(|index| entries[index].clone()));
                }
                CatalogPageBody::Branch { children } => {
                    let index = children.partition_point(|child| child.first_key.as_str() < upper);
                    let Some(index) = index.checked_sub(1) else {
                        return Ok(None);
                    };
                    reference = children[index].clone();
                }
            }
        }
    }

    fn last_entry(&self, root: &CatalogRoot) -> Result<Option<CatalogEntry>> {
        let mut reference = root.root.clone();
        loop {
            match self.load_page(&reference)? {
                CatalogPageBody::Leaf { entries } => return Ok(entries.last().cloned()),
                CatalogPageBody::Branch { children } => {
                    reference = children.last().expect("validated branch").clone();
                }
            }
        }
    }
}
