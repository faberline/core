pub const DEFAULT_CATALOG_PAGE_BYTES: usize = 64 * 1024;
/// Maximum page keys retained by the compatibility abort helper.
///
/// Production builds with larger catalogs must use `build_sorted_observed`
/// and persist page keys outside process memory.
pub const MAX_ABORT_TRACKED_CATALOG_PAGES: usize = 1_024;
pub(crate) const CATALOG_FORMAT_VERSION: u16 = 1;
pub(crate) const CATALOG_PAGE_FORMAT_VERSION: u16 = 1;
