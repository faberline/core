//! Markdown symbol extraction (line-based)
//!
//! Extracts symbols from Markdown documents:
//! - Headings (h1–h6) as Label
//! - Links `[text](url)` as Resource
//! - Code blocks ` ```lang ` as Template
//! - Frontmatter fields (`key: value` between `---`) as Variable
