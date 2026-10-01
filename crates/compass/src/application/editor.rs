//! Editor use cases for the language server: keep the open documents,
//! analyse them (lint, symbols, search index) and answer hover, definition,
//! references, completion and code-action requests.
//!
//! `ArgusServer` (interfaces) translates LSP messages into these calls; the
//! composition root builds the session with the tree-sitter parser and the
//! refactoring engine.
pub(crate) mod completion;
pub(crate) mod refactor_actions;
pub(crate) mod session;
#[cfg(test)]
mod tests;
pub(crate) mod views;
