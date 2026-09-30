//! Public modules that keep their paths: they hold names the crate root does
//! not re-export.

pub mod check_pipeline;
pub mod core;
pub mod diagnostic;
pub mod gen;
pub mod graph;
pub mod lens_error;
pub mod lint;
pub mod lsp;
pub mod output;
pub mod schemas;
pub mod semantic;
pub mod server;
pub mod spec;
pub mod storage;
pub mod syntax;
pub mod type_inference;
