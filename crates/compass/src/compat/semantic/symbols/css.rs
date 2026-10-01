//! CSS symbol extraction (tree-sitter)
//!
//! Extracts symbols from CSS stylesheets:
//! - Class selectors (.name) as Class
//! - ID selectors (#name) as Variable
//! - Custom properties (--var-name) as Variable
//! - @keyframes names as Function
//! - @media descriptors as Label
