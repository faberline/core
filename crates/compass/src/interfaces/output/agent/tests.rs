use super::*;
use crate::checker::FileResult;
use crate::diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, Position, Range};
use crate::graph::ImportGraph;
use crate::semantic::symbols::{SymbolKind, SymbolTable, TypeInfo};
use crate::syntax::Language;
use std::path::PathBuf;

mod build_sections;
mod compact_json;
mod helper_fns;
mod issues_impact;

/// Helper: create a Range from 0-indexed (start_line, start_char) to (end_line, end_char).
fn make_range(sl: u32, sc: u32, el: u32, ec: u32) -> Range {
    Range::new(Position::new(sl, sc), Position::new(el, ec))
}

/// Helper: create a minimal FileResult with given path, language, and diagnostics.
fn make_file_result(path: &str, lang: Language, diagnostics: Vec<Diagnostic>) -> FileResult {
    FileResult {
        path: PathBuf::from(path),
        language: lang,
        diagnostics,
    }
}

/// Helper: create a Diagnostic at a given 0-indexed line.
fn make_diagnostic(
    line: u32,
    severity: DiagnosticSeverity,
    code: &str,
    message: &str,
) -> Diagnostic {
    Diagnostic::new(
        make_range(line, 0, line, 10),
        severity,
        code,
        DiagnosticCategory::Style,
        message,
    )
}

/// Helper: build a SymbolTable with a function symbol.
fn make_symbol_table_with_function(
    name: &str,
    kind: SymbolKind,
    line: u32,
    end_line: u32,
    type_info: Option<TypeInfo>,
) -> SymbolTable {
    let mut table = SymbolTable::new();
    table.add_symbol(
        name.to_string(),
        kind,
        make_range(line, 0, end_line, 0),
        type_info,
        None,
        0,
    );
    table
}
