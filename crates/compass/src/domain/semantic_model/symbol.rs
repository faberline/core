use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::diagnostic::Range;
use crate::domain::semantic_model::ids::{ScopeId, SymbolId};
use crate::domain::semantic_model::type_info::TypeInfo;

/// Kind of symbol in the semantic model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticSymbolKind {
    Variable,
    Function,
    Class,
    Parameter,
    Import,
    Module,
    TypeAlias,
    Attribute,
    Method,
    Property,
}

impl SemanticSymbolKind {
    /// Get a display name for the symbol kind
    pub fn display_name(&self) -> &'static str {
        match self {
            SemanticSymbolKind::Variable => "variable",
            SemanticSymbolKind::Function => "function",
            SemanticSymbolKind::Class => "class",
            SemanticSymbolKind::Parameter => "parameter",
            SemanticSymbolKind::Import => "import",
            SemanticSymbolKind::Module => "module",
            SemanticSymbolKind::TypeAlias => "type alias",
            SemanticSymbolKind::Attribute => "attribute",
            SemanticSymbolKind::Method => "method",
            SemanticSymbolKind::Property => "property",
        }
    }
}

/// Symbol data stored in the semantic model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolData {
    /// Symbol name
    pub name: String,
    /// Kind of symbol
    pub kind: SemanticSymbolKind,
    /// Definition range (where the symbol is defined)
    pub def_range: Range,
    /// File path where the symbol is defined
    pub file_path: PathBuf,
    /// Type information for this symbol
    pub type_info: TypeInfo,
    /// Documentation string if available
    pub documentation: Option<String>,
    /// Scope this symbol belongs to
    pub scope_id: ScopeId,
    /// Parent symbol (for methods/attributes of a class)
    pub parent_id: Option<SymbolId>,
}

/// Reference to a symbol (usage site)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolReference {
    /// The symbol being referenced
    pub symbol_id: SymbolId,
    /// Range of the reference
    pub range: Range,
    /// Whether this reference is the definition
    pub is_definition: bool,
}
