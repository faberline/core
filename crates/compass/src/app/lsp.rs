//! `ArgusServer::new`, `run_server` and `run_server_tcp`: the language
//! server wired to the tree-sitter parser and the refactoring engine.

use tower_lsp::Client;

use crate::app::parser::NoParser;
use crate::application::editor::session::EditorSession;
use crate::domain::syntax::source_parser::SourceParser;
use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::infrastructure::syntax::multi_parser::MultiParser;
use crate::interfaces::lsp::argus_server::ArgusServer;
use crate::interfaces::lsp::transport::{serve_stdio, serve_tcp};

impl ArgusServer {
    /// Create a new Argus server
    pub fn new(client: Client) -> Self {
        let parser: Box<dyn SourceParser + Send + Sync> = match MultiParser::new() {
            Ok(parser) => Box::new(parser),
            Err(_) => Box::new(NoParser),
        };
        Self::with_session(client, EditorSession::new(parser, RefactoringEngine::new()))
    }
}

/// Run the LSP server on stdio
pub async fn run_server() {
    serve_stdio(ArgusServer::new).await;
}

/// Run the LSP server on TCP (for debugging)
pub async fn run_server_tcp(port: u16) -> std::io::Result<()> {
    serve_tcp(port, ArgusServer::new).await
}
