//! `SemanticSearchEngine`'s source-text entry points: they parse with the
//! tree-sitter parser, then hand the parsed file to the domain engine.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::domain::semantic::symbols::SymbolTable;
use crate::domain::semantic_search::engine::SemanticSearchEngine;
use crate::domain::syntax::language::Language;
use crate::domain::syntax::parsed_file::ParsedFile;
use crate::infrastructure::syntax::multi_parser::MultiParser;

/// Parse `content` as `language`.
fn parse(content: &str, language: Language) -> Result<ParsedFile, String> {
    let mut parser = MultiParser::new().map_err(|e| format!("Failed to create parser: {:?}", e))?;
    parser
        .parse(content, language)
        .ok_or_else(|| "Failed to parse file".to_string())
}

impl SemanticSearchEngine {
    /// Build call graph from source code.
    pub fn build_call_graph(
        &mut self,
        file: PathBuf,
        content: &str,
        language: Language,
    ) -> Result<(), String> {
        let parsed = parse(content, language)?;
        self.build_call_graph_parsed(file, &parsed);
        Ok(())
    }

    /// Extract docstrings from source code.
    /// Returns a map of symbol name -> docstring.
    pub fn extract_docstrings(
        &self,
        content: &str,
        language: Language,
    ) -> Result<HashMap<String, String>, String> {
        let parsed = parse(content, language)?;
        Ok(self.extract_docstrings_parsed(&parsed))
    }

    /// Index a symbol table for searching, also extracting docstrings from the
    /// provided source code via AST traversal (R3.4).
    ///
    /// This is the preferred variant when the source text is available.
    pub fn index_symbol_table_with_source(
        &mut self,
        file: PathBuf,
        symbol_table: &SymbolTable,
        source: &str,
        language: Language,
    ) {
        match parse(source, language) {
            Ok(parsed) => self.index_symbol_table_parsed(file, symbol_table, &parsed),
            Err(_) => self.index_symbol_table_with_docstrings(file, symbol_table, &HashMap::new()),
        }
    }
}
