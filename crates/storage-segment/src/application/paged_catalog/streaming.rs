use super::PagedCatalog;
use crate::domain::{
    CatalogEntry, CatalogPageBody, CatalogPageRef, CatalogRoot, Result, SegmentError,
    StreamingCatalogAbort, StreamingCatalogBuild, CATALOG_FORMAT_VERSION,
    MAX_ABORT_TRACKED_CATALOG_PAGES,
};

impl PagedCatalog {
    /// Build a catalog without retaining all entries or page references.
    ///
    /// The caller owns external sorting. This method rejects an equal or
    /// decreasing key. It retains at most one page-sized entry buffer and one
    /// page-sized reference buffer for each tree level.
    pub fn build_sorted(
        &self,
        entries: impl IntoIterator<Item = Result<CatalogEntry>>,
    ) -> Result<StreamingCatalogBuild> {
        self.build_sorted_observed(entries, |_| Ok(()))
    }

    /// Build a sorted catalog and retain page keys for abort cleanup.
    ///
    /// This compatibility API has a hard limit of
    /// `MAX_ABORT_TRACKED_CATALOG_PAGES`. It fails before retaining another
    /// key. Use `build_sorted_observed` for catalogs that can exceed the cap.
    pub fn build_sorted_with_abort(
        &self,
        entries: impl IntoIterator<Item = Result<CatalogEntry>>,
    ) -> std::result::Result<StreamingCatalogBuild, StreamingCatalogAbort> {
        let mut written_page_keys = Vec::new();
        self.build_sorted_observed(entries, |reference| {
            if written_page_keys.len() == MAX_ABORT_TRACKED_CATALOG_PAGES {
                return Err(SegmentError::CorruptCatalog {
                    message: format!(
                        "compatibility abort cleanup reached its bounded limit of {} pages; use build_sorted_observed",
                        MAX_ABORT_TRACKED_CATALOG_PAGES
                    ),
                });
            }
            written_page_keys.push(reference.key.clone());
            Ok(())
        })
        .map_err(|error| StreamingCatalogAbort {
            error,
            written_page_keys,
        })
    }

    /// Build a sorted catalog while reporting each durable page immediately.
    ///
    /// A production caller can persist the keys in a disk-backed ledger. This
    /// keeps abort cleanup bounded by the catalog page size instead of keeping
    /// one `String` per uploaded page in memory. If the observer rejects a
    /// page, this method deletes that just-written page before it returns.
    pub fn build_sorted_observed(
        &self,
        entries: impl IntoIterator<Item = Result<CatalogEntry>>,
        mut observe: impl FnMut(&CatalogPageRef) -> Result<()>,
    ) -> Result<StreamingCatalogBuild> {
        self.build_sorted_inner(entries, &mut observe)
    }

    fn build_sorted_inner<F>(
        &self,
        entries: impl IntoIterator<Item = Result<CatalogEntry>>,
        observe: &mut F,
    ) -> Result<StreamingCatalogBuild>
    where
        F: FnMut(&CatalogPageRef) -> Result<()>,
    {
        let mut leaf = Vec::<CatalogEntry>::new();
        let mut levels = Vec::<Vec<CatalogPageRef>>::new();
        let empty_leaf_bytes = self.page_size(&CatalogPageBody::Leaf {
            entries: Vec::new(),
        })?;
        let empty_branch_bytes = self.page_size(&CatalogPageBody::Branch {
            children: Vec::new(),
        })?;
        let mut leaf_bytes = empty_leaf_bytes;
        let mut level_bytes = Vec::<usize>::new();
        let mut last_key = None::<String>;
        let mut entry_count = 0_u64;
        let mut written_page_count = 0_u64;
        let mut peak_buffer_bytes = 0_usize;

        for entry in entries {
            let entry = entry?;
            self.validate_entry_key(&entry.key)?;
            if last_key
                .as_ref()
                .is_some_and(|previous| previous >= &entry.key)
            {
                return Err(SegmentError::UnsortedCatalogKey { key: entry.key });
            }
            last_key = Some(entry.key.clone());
            entry_count =
                entry_count
                    .checked_add(1)
                    .ok_or_else(|| SegmentError::CorruptCatalog {
                        message: "catalog entry count exhausted u64".to_string(),
                    })?;
            let encoded_entry =
                serde_json::to_vec(&entry).map_err(|error| SegmentError::Serialization {
                    message: error.to_string(),
                })?;
            let added = encoded_entry.len() + usize::from(!leaf.is_empty());
            if !leaf.is_empty() && leaf_bytes.saturating_add(added) > self.page_bytes_limit {
                let reference = self.store_page(CatalogPageBody::Leaf {
                    entries: std::mem::take(&mut leaf),
                })?;
                self.observe_streaming_page(observe, &reference)?;
                written_page_count = written_page_count.saturating_add(1);
                self.push_streaming_ref(
                    &mut levels,
                    &mut level_bytes,
                    empty_branch_bytes,
                    0,
                    reference,
                    &mut written_page_count,
                    observe,
                )?;
                leaf_bytes = empty_leaf_bytes;
            }
            let added = encoded_entry.len() + usize::from(!leaf.is_empty());
            if leaf_bytes.saturating_add(added) > self.page_bytes_limit {
                return Err(SegmentError::CatalogPageTooLarge {
                    limit: self.page_bytes_limit,
                });
            }
            leaf_bytes = leaf_bytes.saturating_add(added);
            leaf.push(entry);
            peak_buffer_bytes = peak_buffer_bytes
                .max(leaf_bytes.saturating_add(level_bytes.iter().copied().sum::<usize>()));
        }

        if entry_count == 0 {
            let root = self.store_page(CatalogPageBody::Leaf {
                entries: Vec::new(),
            })?;
            self.observe_streaming_page(observe, &root)?;
            return Ok(StreamingCatalogBuild {
                root: CatalogRoot {
                    format_version: CATALOG_FORMAT_VERSION,
                    height: 0,
                    entry_count: 0,
                    page_bytes_limit: self.page_bytes_limit as u32,
                    root,
                },
                written_page_count: 1,
                peak_buffer_bytes,
            });
        }

        let reference = self.store_page(CatalogPageBody::Leaf { entries: leaf })?;
        self.observe_streaming_page(observe, &reference)?;
        written_page_count = written_page_count.saturating_add(1);
        self.push_streaming_ref(
            &mut levels,
            &mut level_bytes,
            empty_branch_bytes,
            0,
            reference,
            &mut written_page_count,
            observe,
        )?;
        peak_buffer_bytes = peak_buffer_bytes.max(level_bytes.iter().copied().sum::<usize>());

        let (root_ref, height) = loop {
            let non_empty = levels
                .iter()
                .enumerate()
                .filter(|(_, level)| !level.is_empty())
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if non_empty.len() == 1 && levels[non_empty[0]].len() == 1 {
                let height =
                    u16::try_from(non_empty[0]).map_err(|_| SegmentError::CorruptCatalog {
                        message: "catalog height exhausted u16".to_string(),
                    })?;
                break (
                    levels[non_empty[0]].pop().expect("one root reference"),
                    height,
                );
            }
            let level = *non_empty.first().expect("streaming catalog has references");
            let children = std::mem::take(&mut levels[level]);
            if children.len() < 2 {
                return Err(SegmentError::CorruptCatalog {
                    message: "streaming catalog would create a unary branch".to_string(),
                });
            }
            level_bytes[level] = empty_branch_bytes;
            let reference = self.store_page(CatalogPageBody::Branch { children })?;
            self.observe_streaming_page(observe, &reference)?;
            written_page_count = written_page_count.saturating_add(1);
            self.push_streaming_ref(
                &mut levels,
                &mut level_bytes,
                empty_branch_bytes,
                level + 1,
                reference,
                &mut written_page_count,
                observe,
            )?;
            peak_buffer_bytes = peak_buffer_bytes.max(level_bytes.iter().copied().sum::<usize>());
        };
        let root = CatalogRoot {
            format_version: CATALOG_FORMAT_VERSION,
            height,
            entry_count,
            page_bytes_limit: self.page_bytes_limit as u32,
            root: root_ref,
        };
        self.validate_root(&root)?;
        Ok(StreamingCatalogBuild {
            root,
            written_page_count,
            peak_buffer_bytes,
        })
    }

    fn push_streaming_ref<F>(
        &self,
        levels: &mut Vec<Vec<CatalogPageRef>>,
        level_bytes: &mut Vec<usize>,
        empty_branch_bytes: usize,
        level: usize,
        reference: CatalogPageRef,
        written_page_count: &mut u64,
        observe: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&CatalogPageRef) -> Result<()>,
    {
        if levels.len() <= level {
            levels.resize_with(level + 1, Vec::new);
            level_bytes.resize(level + 1, empty_branch_bytes);
        }
        let encoded_reference =
            serde_json::to_vec(&reference).map_err(|error| SegmentError::Serialization {
                message: error.to_string(),
            })?;
        let added = encoded_reference.len() + usize::from(!levels[level].is_empty());
        if levels[level].is_empty()
            && level_bytes[level].saturating_add(added) > self.page_bytes_limit
        {
            return Err(SegmentError::CatalogPageTooLarge {
                limit: self.page_bytes_limit,
            });
        }
        if !levels[level].is_empty()
            && level_bytes[level].saturating_add(added) > self.page_bytes_limit
        {
            if levels[level].len() < 3 {
                return Err(SegmentError::CatalogPageTooLarge {
                    limit: self.page_bytes_limit,
                });
            }
            let mut children = std::mem::take(&mut levels[level]);
            let retained = children
                .pop()
                .expect("streaming level has at least three references");
            let parent = self.store_page(CatalogPageBody::Branch { children })?;
            self.observe_streaming_page(observe, &parent)?;
            let retained_bytes = serde_json::to_vec(&retained)
                .map_err(|error| SegmentError::Serialization {
                    message: error.to_string(),
                })?
                .len();
            levels[level].push(retained);
            level_bytes[level] = empty_branch_bytes.saturating_add(retained_bytes);
            *written_page_count = written_page_count.saturating_add(1);
            self.push_streaming_ref(
                levels,
                level_bytes,
                empty_branch_bytes,
                level + 1,
                parent,
                written_page_count,
                observe,
            )?;
        }
        level_bytes[level] = level_bytes[level]
            .saturating_add(encoded_reference.len() + usize::from(!levels[level].is_empty()));
        levels[level].push(reference);
        Ok(())
    }

    fn observe_streaming_page(
        &self,
        observe: &mut impl FnMut(&CatalogPageRef) -> Result<()>,
        reference: &CatalogPageRef,
    ) -> Result<()> {
        if let Err(error) = observe(reference) {
            self.objects.delete(&reference.key)?;
            return Err(error);
        }
        Ok(())
    }
}
