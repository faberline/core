use std::path::PathBuf;

use crate::domain::semantic_search::engine::{SemanticSearchEngine, SymbolLocation};
use crate::semantic::SymbolTable;
use crate::type_inference::Type;

impl SemanticSearchEngine {
    /// Index a file for searching.
    pub fn index_file(&mut self, file: PathBuf, symbols: Vec<SymbolLocation>) {
        for sym in symbols {
            if let Some(name) = self.extract_symbol_name(&sym) {
                self.symbol_index.entry(name).or_default().push(sym.clone());
            }

            // Index by type signature if it's a callable
            if let Some(ref ty) = sym.ty {
                if matches!(ty, Type::Callable { .. }) {
                    let sig_key = self.compute_signature_key(ty);
                    self.type_signature_index
                        .entry(sig_key)
                        .or_default()
                        .push(sym.clone());
                }
            }
        }
        self.inferencer.add_file(file);
    }

    /// Compute a signature key for indexing.
    fn compute_signature_key(&self, ty: &Type) -> String {
        match ty {
            Type::Callable { params, ret } => {
                let params_str = params
                    .iter()
                    .map(|p| self.type_to_key_string(&p.ty))
                    .collect::<Vec<_>>()
                    .join(",");
                let ret_str = self.type_to_key_string(ret);
                format!("({}) -> {}", params_str, ret_str)
            }
            _ => "unknown".to_string(),
        }
    }

    /// Convert type to a string key for indexing.
    fn type_to_key_string(&self, ty: &Type) -> String {
        match ty {
            Type::Int => "int".to_string(),
            Type::Str => "str".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Float => "float".to_string(),
            Type::None => "None".to_string(),
            Type::Any => "Any".to_string(),
            Type::Unknown => "Unknown".to_string(),
            Type::List(elem) => format!("List[{}]", self.type_to_key_string(elem)),
            Type::Dict(k, v) => format!(
                "Dict[{}, {}]",
                self.type_to_key_string(k),
                self.type_to_key_string(v)
            ),
            Type::Set(elem) => format!("Set[{}]", self.type_to_key_string(elem)),
            Type::Tuple(elems) => {
                let elems_str = elems
                    .iter()
                    .map(|e| self.type_to_key_string(e))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Tuple[{}]", elems_str)
            }
            Type::Union(types) => {
                let types_str = types
                    .iter()
                    .map(|t| self.type_to_key_string(t))
                    .collect::<Vec<_>>()
                    .join(" | ");
                types_str
            }
            Type::Optional(inner) => format!("{}?", self.type_to_key_string(inner)),
            Type::Instance { name, .. } => name.clone(),
            Type::ClassType { name, .. } => format!("type[{}]", name),
            _ => "Unknown".to_string(),
        }
    }

    /// Index a symbol table for searching.
    pub fn index_symbol_table(&mut self, file: PathBuf, symbol_table: &SymbolTable) {
        let symbols = self.convert_symbol_table_to_locations(file.clone(), symbol_table);
        self.index_file(file, symbols);
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
        language: crate::syntax::Language,
    ) {
        let mut symbols = self.convert_symbol_table_to_locations(file.clone(), symbol_table);

        // Extract docstrings from the AST and attach them to matching symbols
        if let Ok(docstrings) = self.extract_docstrings(source, language) {
            for loc in &mut symbols {
                if let Some(doc) = docstrings.get(&loc.name) {
                    loc.docstring = Some(doc.clone());
                }
            }
        }

        self.index_file(file, symbols);
    }

    /// Update docstrings for indexed symbols.
    pub fn update_docstrings(&mut self, docstrings: std::collections::HashMap<String, String>) {
        for (symbol_name, docstring) in docstrings {
            if let Some(locations) = self.symbol_index.get_mut(&symbol_name) {
                for location in locations {
                    location.docstring = Some(docstring.clone());
                }
            }
        }
    }
}
