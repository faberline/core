//! Dockerfile symbol extraction (line-based)
//!
//! Extracts symbols from Dockerfile source without tree-sitter:
//! - FROM stages: `FROM image AS name`
//! - ENV vars: `ENV KEY=value` or `ENV KEY value`
//! - ARG declarations: `ARG NAME=default`
//! - EXPOSE ports: `EXPOSE 8080`
//! - LABEL keys: `LABEL key=value`
