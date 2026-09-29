use std::path::PathBuf;

use crate::domain::ast_editing::mutable_ast::Span;
use crate::domain::semantic_search::engine::{SemanticSearchEngine, SymbolLocation};
use crate::domain::semantic_search::result::MatchKind;
use crate::semantic::{SymbolKind as SemanticSymbolKind, SymbolTable};
use crate::type_inference::Type;

impl SemanticSearchEngine {
    /// Convert a SymbolTable to a Vec of SymbolLocations.
    pub(super) fn convert_symbol_table_to_locations(
        &self,
        file: PathBuf,
        symbol_table: &SymbolTable,
    ) -> Vec<SymbolLocation> {
        let mut locations = Vec::new();

        for symbol in symbol_table.all_symbols() {
            let kind = Self::convert_symbol_kind(symbol.kind);
            let ty = symbol
                .type_info
                .as_ref()
                .map(|ti| Self::convert_type_info(ti));

            locations.push(SymbolLocation {
                file: file.clone(),
                span: Span {
                    start: 0, // We don't have byte offsets in semantic::Symbol
                    end: 0,
                    start_line: symbol.location.start.line as usize,
                    start_col: symbol.location.start.character as usize,
                    end_line: symbol.location.end.line as usize,
                    end_col: symbol.location.end.character as usize,
                },
                name: symbol.name.clone(),
                kind,
                ty,
                docstring: None, // TODO: Extract from AST
            });
        }

        locations
    }

    /// Convert semantic SymbolKind to search MatchKind.
    fn convert_symbol_kind(kind: SemanticSymbolKind) -> MatchKind {
        match kind {
            SemanticSymbolKind::Function => MatchKind::FunctionDef,
            SemanticSymbolKind::Class | SemanticSymbolKind::Struct => MatchKind::ClassDef,
            SemanticSymbolKind::Variable
            | SemanticSymbolKind::Const
            | SemanticSymbolKind::Static => MatchKind::VariableAssignment,
            SemanticSymbolKind::Parameter => MatchKind::VariableAssignment,
            SemanticSymbolKind::Import => MatchKind::Import,
            SemanticSymbolKind::TypeAlias => MatchKind::TypeAnnotation,
            SemanticSymbolKind::Interface | SemanticSymbolKind::Trait => MatchKind::ClassDef,
            _ => MatchKind::VariableAssignment,
        }
    }

    /// Convert semantic TypeInfo to Type.
    pub(super) fn convert_type_info(type_info: &crate::semantic::TypeInfo) -> Type {
        use crate::semantic::TypeInfo;
        use crate::type_inference::{Param, ParamKind};

        match type_info {
            TypeInfo::Primitive(name) => match name.as_str() {
                "int" => Type::Int,
                "str" => Type::Str,
                "bool" => Type::Bool,
                "float" => Type::Float,
                "None" => Type::None,
                _ => Type::Unknown,
            },
            TypeInfo::List(inner) => {
                let elem = Self::convert_type_info(inner);
                Type::List(Box::new(elem))
            }
            TypeInfo::Dict(key, value) => {
                let key_ty = Self::convert_type_info(key);
                let value_ty = Self::convert_type_info(value);
                Type::Dict(Box::new(key_ty), Box::new(value_ty))
            }
            TypeInfo::Optional(inner) => {
                let inner_ty = Self::convert_type_info(inner);
                Type::Union(vec![inner_ty, Type::None])
            }
            TypeInfo::Union(types) => {
                let converted_types = types.iter().map(Self::convert_type_info).collect();
                Type::Union(converted_types)
            }
            TypeInfo::Callable { params, ret } => {
                // Convert TypeInfo params to Param structs
                let param_structs: Vec<Param> = params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| Param {
                        name: format!("arg{}", i),
                        ty: Self::convert_type_info(p),
                        has_default: false,
                        kind: ParamKind::Positional,
                    })
                    .collect();
                let return_type = Box::new(Self::convert_type_info(ret));
                Type::Callable {
                    params: param_structs,
                    ret: return_type,
                }
            }
            TypeInfo::Named(name) => Type::Instance {
                name: name.clone(),
                module: None,
                type_args: vec![],
            },
            TypeInfo::Generic(name, args) => {
                let type_args = args.iter().map(Self::convert_type_info).collect();
                Type::Instance {
                    name: name.clone(),
                    module: None,
                    type_args,
                }
            }
            TypeInfo::Reference(inner) => Self::convert_type_info(inner),
            TypeInfo::Unknown => Type::Unknown,
            TypeInfo::Any => Type::Any,
            TypeInfo::Error => Type::Unknown,
        }
    }

    pub(super) fn extract_symbol_name(&self, sym: &SymbolLocation) -> Option<String> {
        Some(sym.name.clone())
    }
}
