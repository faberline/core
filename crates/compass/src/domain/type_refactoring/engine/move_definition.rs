use std::path::PathBuf;

use crate::domain::type_refactoring::engine::RefactoringEngine;
use crate::domain::type_refactoring::request::RefactorRequest;
use crate::domain::type_refactoring::result::{DiagnosticLevel, RefactorResult, TextEdit};

impl RefactoringEngine {
    /// Move a definition to another file.
    pub(super) fn move_definition(
        &mut self,
        request: &RefactorRequest,
        target_file: &PathBuf,
        source: &str,
    ) -> RefactorResult {
        let mut result = RefactorResult::empty();

        // Get or populate AST
        if let Err(e) = self.populate_ast_cache(&request.file, source) {
            result.add_diagnostic(
                DiagnosticLevel::Error,
                format!("Failed to parse file: {}", e),
                Some(request.file.clone()),
                None,
            );
            return result;
        }

        // Extract the definition code
        let definition_code = &source[request.span.start..request.span.end];

        // Remove from current file
        result.add_edit(
            request.file.clone(),
            TextEdit {
                span: request.span,
                new_text: String::new(),
            },
        );

        // Add to target file (at beginning for simplicity)
        result
            .new_files
            .insert(target_file.clone(), format!("{}\n\n", definition_code));

        result.add_diagnostic(
            DiagnosticLevel::Info,
            format!("Moved definition to {:?}", target_file),
            Some(request.file.clone()),
            Some(request.span),
        );

        result
    }
}
