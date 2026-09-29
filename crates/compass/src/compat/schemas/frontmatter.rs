//! Frontmatter schema definitions for Hugo, Jekyll, Docusaurus, and Generic.
//!
//! Provides JSON Schema validators for YAML frontmatter used in static site
//! generators. Validators are compiled once and cached via `OnceLock`.

pub use crate::infrastructure::schema_validation::frontmatter::{
    detect_framework, frontmatter_validator, FrontmatterFramework,
};
