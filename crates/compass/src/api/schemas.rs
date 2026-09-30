//! Schema-based validation for Kubernetes manifests and GitLab CI configs.
//!
//! Builds JSON Schemas programmatically in Rust (no bundled JSON files)
//! and validates parsed YAML/JSON values against them using the `jsonschema` crate.

pub mod frontmatter;
mod gitlab;
mod k8s;

pub use crate::infrastructure::schema_validation::registry::SchemaRegistry;
