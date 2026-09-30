use std::path::PathBuf;
use std::sync::Arc;

use tower_lsp::lsp_types::*;
use tower_lsp::Client;

use crate::application::editor::session::EditorSession;
use crate::domain::diagnostic::model::{
    Diagnostic as ArgusDiagnostic, DiagnosticSeverity as ArgusSeverity, Position as ArgusPosition,
    Range as ArgusRange,
};

/// Argus Language Server
pub struct ArgusServer {
    pub(super) client: Client,
    pub(super) session: Arc<EditorSession>,
}

impl ArgusServer {
    /// A server over an editor session. `ArgusServer::new` (in the
    /// composition root) builds the session with the tree-sitter parser.
    pub(crate) fn with_session(client: Client, session: EditorSession) -> Self {
        Self {
            client,
            session: Arc::new(session),
        }
    }

    /// Analyze a document and publish diagnostics
    pub(super) async fn analyze_document(&self, uri: &Url) {
        let Some(diagnostics) = self.session.analyse(uri.as_str()).await else {
            return;
        };

        // Convert to LSP diagnostics
        let lsp_diagnostics: Vec<tower_lsp::lsp_types::Diagnostic> = diagnostics
            .iter()
            .map(|d| self.to_lsp_diagnostic(d))
            .collect();

        // Publish diagnostics
        self.client
            .publish_diagnostics(uri.clone(), lsp_diagnostics, None)
            .await;
    }

    /// Convert Argus diagnostic to LSP diagnostic
    pub(super) fn to_lsp_diagnostic(
        &self,
        diag: &ArgusDiagnostic,
    ) -> tower_lsp::lsp_types::Diagnostic {
        tower_lsp::lsp_types::Diagnostic {
            range: Self::to_lsp_range(&diag.range),
            severity: Some(self.to_lsp_severity(diag.severity)),
            code: Some(NumberOrString::String(diag.code.to_string())),
            code_description: None,
            source: Some("cclab_lens".to_string()),
            message: diag.message.clone(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    /// Convert Argus severity to LSP severity
    fn to_lsp_severity(&self, severity: ArgusSeverity) -> DiagnosticSeverity {
        match severity {
            ArgusSeverity::Error => DiagnosticSeverity::ERROR,
            ArgusSeverity::Warning => DiagnosticSeverity::WARNING,
            ArgusSeverity::Information => DiagnosticSeverity::INFORMATION,
            ArgusSeverity::Hint => DiagnosticSeverity::HINT,
        }
    }

    /// The file path of a document URI
    pub(super) fn document_path(uri: &Url) -> PathBuf {
        PathBuf::from(uri.path())
    }

    /// Convert Argus Range to LSP Range
    pub(super) fn to_lsp_range(range: &ArgusRange) -> Range {
        Range {
            start: Position {
                line: range.start.line,
                character: range.start.character,
            },
            end: Position {
                line: range.end.line,
                character: range.end.character,
            },
        }
    }

    /// Convert LSP Range to Argus Range
    pub(super) fn from_lsp_range(range: &Range) -> ArgusRange {
        ArgusRange::new(
            ArgusPosition::new(range.start.line, range.start.character),
            ArgusPosition::new(range.end.line, range.end.character),
        )
    }
}
