//! # compass
//!
//! Code intelligence arsenal for the cclab ecosystem. Compass gives developers
//! and AI agents the ability to **navigate** a codebase — tree-sitter parsing,
//! type inference, semantic analysis, LSP integration, file watching,
//! refactoring, and lint infrastructure.
//!
//! ## Naming
//!
//! "Compass" = navigation. Code intelligence is about finding your way
//! through an unfamiliar codebase: jump to definition, find references,
//! impact analysis, dependency graph. The tool is the compass; the
//! codebase is the terrain.
//!
//! ## Consumers
//!
//! - `apps/agentic-workflow/` — local Rust CLI (direct dependency)
//! - `projects/conductor/` — cloud web
//! - `sdd` — library crate re-exports compass for backward compat

mod api;
mod application;
mod domain;
mod infrastructure;
mod interfaces;

pub use api::{
    check_pipeline, core, diagnostic, gen, graph, lens_error, lint, lsp, output, schemas, semantic,
    server, spec, storage, syntax, type_inference,
};
// generate/ module moved to sdd crate (consolidate-codegen)

// Re-export commonly used types (matches the surface previously exposed by sdd)
pub use application::analysis::request_handler::RequestHandler;
pub use application::check::check_paths::{check_paths, check_paths_with_propagation};
pub use application::daemon::config::DaemonConfig;
pub use application::outline::function_outline::{
    outline, outline_parsed, FunctionDef, FunctionKind,
};
pub use domain::check::file_result::FileResult;
pub use domain::check::lint_config::LintConfig;
pub use domain::config::argus_config::{ArgusConfig, LanguageConfig};
pub use domain::diagnostic::model::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range,
};
pub use domain::error::argus_error::ArgusError;
pub use domain::lint::checker::Checker;
pub use domain::lint::registry::CheckerRegistry;
pub use domain::syntax::language::Language;
pub use domain::syntax::parsed_file::{NodeRange, ParsedFile};
pub use infrastructure::codegen::traits::{
    CodeGenerator, GenContext, GenError, GenResult, GeneratedCode, TechStack,
};
pub use infrastructure::daemon::client::DaemonClient;
pub use infrastructure::syntax::multi_parser::MultiParser;
pub use infrastructure::watch::file_watcher::{FileWatcher, WatchConfig, WatchEvent};
pub use interfaces::daemon::argus_daemon::ArgusDaemon;
pub use interfaces::output::output_format::OutputFormat;
pub use interfaces::output::reporter::Reporter;
