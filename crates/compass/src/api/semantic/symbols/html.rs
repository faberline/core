//! HTML symbol extraction (tree-sitter)
//!
//! Extracts symbols from HTML documents:
//! - Element IDs (id="...") as Variable
//! - Class names (class="...") as Class
//! - Form names (name="...") as Variable
//! - Anchor hrefs as references
//! - Meta tag names as Label
