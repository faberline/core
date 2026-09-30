//! What the reporters render besides the check results: symbol tables and
//! the import graph, as views.
//!
//! The views keep what the agent output uses: the user-defined symbols (no
//! imports or parameters) with their category, type signature and range,
//! the references to them, and the import paths of each checked file. The
//! composition root builds them for `AgentOutputBuilder::build` and
//! `Reporter::generate_agent`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::domain::diagnostic::model::Range;
use crate::domain::import_graph::graph::ImportGraph;
use crate::domain::semantic::symbols::{SymbolKind, SymbolTable};

/// The category of a symbol in the agent output schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolCategory {
    Function,
    Class,
    Interface,
    Variable,
    Constant,
    TypeAlias,
    Module,
}

impl SymbolCategory {
    /// The category of a symbol kind.
    pub(crate) fn of(kind: SymbolKind) -> Self {
        match kind {
            SymbolKind::Function => Self::Function,
            SymbolKind::Class | SymbolKind::Struct | SymbolKind::Enum => Self::Class,
            SymbolKind::Trait | SymbolKind::Interface => Self::Interface,
            SymbolKind::Variable => Self::Variable,
            SymbolKind::Const | SymbolKind::Static => Self::Constant,
            SymbolKind::TypeAlias | SymbolKind::TypeParameter => Self::TypeAlias,
            SymbolKind::Module | SymbolKind::Impl => Self::Module,
            // Infrastructure and other kinds default to "variable"
            SymbolKind::Resource
            | SymbolKind::Job
            | SymbolKind::Stage
            | SymbolKind::Port
            | SymbolKind::Label
            | SymbolKind::Selector
            | SymbolKind::Template => Self::Variable,
            // Decorators, macros
            SymbolKind::Decorator | SymbolKind::Macro => Self::Function,
            // Imports and parameters never reach the views (see
            // is_user_defined); enum members count as variables
            SymbolKind::Import | SymbolKind::Parameter | SymbolKind::EnumMember => Self::Variable,
        }
    }
}

/// A user-defined symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SymbolView {
    pub(crate) name: String,
    pub(crate) category: SymbolCategory,
    /// The displayed type, when the symbol table has one
    pub(crate) type_signature: Option<String>,
    pub(crate) location: Range,
}

/// A reference to a user-defined symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReferenceView {
    pub(crate) symbol_name: String,
    pub(crate) location: Range,
    pub(crate) is_definition: bool,
}

/// A file's symbol table as the reporters see it, in symbol table order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SymbolTableView {
    pub(crate) symbols: Vec<SymbolView>,
    pub(crate) references: Vec<ReferenceView>,
}

impl SymbolTableView {
    /// The view of a symbol table.
    pub(crate) fn of(table: &SymbolTable) -> Self {
        let symbols = table
            .all_symbols()
            .iter()
            .filter(|sym| is_user_defined(sym.kind))
            .map(|sym| SymbolView {
                name: sym.name.clone(),
                category: SymbolCategory::of(sym.kind),
                type_signature: sym.type_info.as_ref().map(|t| t.display()),
                location: sym.location,
            })
            .collect();

        let references = table
            .all_references()
            .iter()
            .filter_map(|reference| {
                let sym = table.get(reference.symbol_id)?;
                is_user_defined(sym.kind).then(|| ReferenceView {
                    symbol_name: sym.name.clone(),
                    location: reference.location,
                    is_definition: reference.is_definition,
                })
            })
            .collect();

        Self {
            symbols,
            references,
        }
    }
}

/// Imports and parameters are not user-defined symbols.
fn is_user_defined(kind: SymbolKind) -> bool {
    !matches!(kind, SymbolKind::Import | SymbolKind::Parameter)
}

/// The import paths of the checked files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ImportGraphView {
    imports: HashMap<PathBuf, Vec<String>>,
}

impl ImportGraphView {
    /// The import paths of `files` in `graph`.
    pub(crate) fn of<'a>(graph: &ImportGraph, files: impl IntoIterator<Item = &'a Path>) -> Self {
        let imports = files
            .into_iter()
            .map(|file| {
                let paths = graph
                    .dependencies(file)
                    .iter()
                    .map(|edge| edge.import_path.clone())
                    .collect();
                (file.to_path_buf(), paths)
            })
            .collect();
        Self { imports }
    }

    /// The import paths of a file; empty when it imports nothing or was not
    /// checked.
    pub(crate) fn imports(&self, file: &Path) -> &[String] {
        self.imports.get(file).map(Vec::as_slice).unwrap_or(&[])
    }
}
