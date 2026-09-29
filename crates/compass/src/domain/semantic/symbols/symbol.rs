use super::TypeInfo;
use crate::domain::diagnostic::model::Range;
use crate::domain::syntax::language::Language;

/// Unique identifier for a symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub usize);

/// Kind of symbol (cross-language)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    // Common
    Variable,
    Function,
    Class,
    Parameter,
    Import,
    Module,

    // Python-specific
    TypeAlias,
    Decorator,

    // TypeScript-specific
    Interface,
    TypeParameter,
    Enum,
    EnumMember,

    // Rust-specific
    Struct,
    Trait,
    Impl,
    Macro,
    Const,
    Static,

    // Infrastructure-specific (Dockerfile, Terraform, K8s, CI)
    Resource,
    Stage,
    Job,
    Port,
    Label,
    Selector,
    Template,
}

impl SymbolKind {
    /// Get LSP symbol kind for hover display
    pub fn display_name(&self) -> &'static str {
        match self {
            SymbolKind::Variable => "variable",
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Parameter => "parameter",
            SymbolKind::Import => "import",
            SymbolKind::Module => "module",
            SymbolKind::TypeAlias => "type alias",
            SymbolKind::Decorator => "decorator",
            SymbolKind::Interface => "interface",
            SymbolKind::TypeParameter => "type parameter",
            SymbolKind::Enum => "enum",
            SymbolKind::EnumMember => "enum member",
            SymbolKind::Struct => "struct",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Macro => "macro",
            SymbolKind::Const => "const",
            SymbolKind::Static => "static",
            SymbolKind::Resource => "resource",
            SymbolKind::Stage => "stage",
            SymbolKind::Job => "job",
            SymbolKind::Port => "port",
            SymbolKind::Label => "label",
            SymbolKind::Selector => "selector",
            SymbolKind::Template => "template",
        }
    }
}

/// A symbol in the symbol table
#[derive(Debug, Clone)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub location: Range,
    pub type_info: Option<TypeInfo>,
    pub doc: Option<String>,
    pub scope_id: usize,
}

impl Symbol {
    /// Generate hover content for this symbol
    pub fn hover_content(&self, language: Language) -> String {
        let mut content = String::new();

        // Add code block with symbol signature
        let lang_str = language.as_str();

        content.push_str(&format!("```{}\n", lang_str));

        match self.kind {
            SymbolKind::Function => {
                if let Some(ref type_info) = self.type_info {
                    if language == Language::Rust {
                        content.push_str(&format!(
                            "fn {}(...) -> {}\n",
                            self.name,
                            type_info.display()
                        ));
                    } else {
                        content.push_str(&format!(
                            "def {}(...) -> {}\n",
                            self.name,
                            type_info.display()
                        ));
                    }
                } else if language == Language::Rust {
                    content.push_str(&format!("fn {}(...)\n", self.name));
                } else {
                    content.push_str(&format!("def {}(...)\n", self.name));
                }
            }
            SymbolKind::Struct => {
                content.push_str(&format!("struct {}\n", self.name));
            }
            SymbolKind::Trait => {
                content.push_str(&format!("trait {}\n", self.name));
            }
            SymbolKind::Impl => {
                content.push_str(&format!("impl {}\n", self.name));
            }
            SymbolKind::Enum => {
                content.push_str(&format!("enum {}\n", self.name));
            }
            SymbolKind::Class => {
                content.push_str(&format!("class {}\n", self.name));
            }
            SymbolKind::Variable | SymbolKind::Parameter => {
                if let Some(ref type_info) = self.type_info {
                    content.push_str(&format!("{}: {}\n", self.name, type_info.display()));
                } else {
                    content.push_str(&format!("{}\n", self.name));
                }
            }
            SymbolKind::Const | SymbolKind::Static => {
                if let Some(ref type_info) = self.type_info {
                    content.push_str(&format!(
                        "{} {}: {}\n",
                        self.kind.display_name(),
                        self.name,
                        type_info.display()
                    ));
                } else {
                    content.push_str(&format!("{} {}\n", self.kind.display_name(), self.name));
                }
            }
            _ => {
                content.push_str(&format!("{} {}\n", self.kind.display_name(), self.name));
            }
        }

        content.push_str("```\n");

        // Add documentation if available
        if let Some(ref doc) = self.doc {
            content.push_str("\n---\n\n");
            content.push_str(doc);
        }

        content
    }
}

/// Reference to a symbol
#[derive(Debug, Clone)]
pub struct SymbolReference {
    pub symbol_id: SymbolId,
    pub location: Range,
    pub is_definition: bool,
}
