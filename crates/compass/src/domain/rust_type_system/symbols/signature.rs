use tree_sitter::Node;

use crate::domain::rust_type_system::symbols::RustSymbolCollector;
use crate::domain::rust_type_system::types::{
    ImplMethod, RustParam, RustType, RustTypeParam, SelfParam, TraitMethod,
};
use crate::type_inference::TypeVarId;

impl RustSymbolCollector {
    pub(super) fn collect_impl_methods(&self, node: &Node, source: &str) -> Vec<ImplMethod> {
        let mut methods = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "function_item" {
                    let name = child
                        .child_by_field_name("name")
                        .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                        .unwrap_or_default();
                    let visibility = self.get_visibility(&child);
                    let type_params = self.collect_method_type_params(&child, source);
                    let params = self.collect_function_params(&child, source);
                    let where_bounds = self.collect_where_clause(&child, source);

                    methods.push(ImplMethod {
                        name: name.clone(),
                        visibility,
                        signature: TraitMethod {
                            name,
                            type_params,
                            self_param: self.get_self_param(&child),
                            params,
                            return_type: self.collect_return_type(&child, source),
                            where_bounds,
                            is_unsafe: child.child_by_field_name("unsafe").is_some(),
                            is_async: child.child_by_field_name("async").is_some(),
                        },
                        is_default: false,
                    });
                }
            }
        }
        methods
    }

    fn collect_method_type_params(&self, node: &Node, source: &str) -> Vec<RustTypeParam> {
        let mut params = Vec::new();
        if let Some(params_node) = node.child_by_field_name("type_parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                match child.kind() {
                    "type_identifier" => {
                        let name = source[child.start_byte()..child.end_byte()].to_string();
                        let id = TypeVarId(params.len());
                        params.push(RustTypeParam {
                            name,
                            id,
                            bounds: vec![],
                            default: None,
                        });
                    }
                    "constrained_type_parameter" => {
                        // T: Bound
                        if let Some(name_node) = child.child_by_field_name("left") {
                            let name =
                                source[name_node.start_byte()..name_node.end_byte()].to_string();
                            let id = TypeVarId(params.len());

                            let mut bounds = Vec::new();
                            if let Some(bounds_node) = child.child_by_field_name("bounds") {
                                bounds = self.parse_trait_bounds(&bounds_node, source);
                            }

                            params.push(RustTypeParam {
                                name,
                                id,
                                bounds,
                                default: None,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        params
    }

    fn get_self_param(&self, node: &Node) -> Option<SelfParam> {
        if let Some(params) = node.child_by_field_name("parameters") {
            let mut cursor = params.walk();
            for child in params.children(&mut cursor) {
                if child.kind() == "self_parameter" {
                    // Check for & or &mut
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "&" {
                            if child.child_by_field_name("mutable_specifier").is_some() {
                                return Some(SelfParam::RefMut(None));
                            } else {
                                return Some(SelfParam::Ref(None));
                            }
                        }
                    }
                    return Some(SelfParam::Value);
                }
            }
        }
        None
    }

    pub(super) fn collect_function_params(&self, node: &Node, source: &str) -> Vec<RustParam> {
        let mut params = Vec::new();
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                if child.kind() == "parameter" {
                    let name = child
                        .child_by_field_name("pattern")
                        .map(|n| source[n.start_byte()..n.end_byte()].to_string())
                        .unwrap_or_default();
                    let ty = child
                        .child_by_field_name("type")
                        .map(|n| self.parse_type(&n, source))
                        .unwrap_or(RustType::Infer);
                    params.push(RustParam {
                        name,
                        ty,
                        is_pattern: false,
                    });
                }
            }
        }
        params
    }

    pub(super) fn collect_return_type(&self, node: &Node, source: &str) -> RustType {
        node.child_by_field_name("return_type")
            .map(|n| self.parse_type(&n, source))
            .unwrap_or(RustType::Unit)
    }

    pub(super) fn parse_type(&self, node: &Node, source: &str) -> RustType {
        let kind = node.kind();
        match kind {
            "primitive_type" => {
                let text = &source[node.start_byte()..node.end_byte()];
                match text {
                    "bool" => RustType::Bool,
                    "char" => RustType::Char,
                    "str" => RustType::Str,
                    "i8" => RustType::I8,
                    "i16" => RustType::I16,
                    "i32" => RustType::I32,
                    "i64" => RustType::I64,
                    "i128" => RustType::I128,
                    "isize" => RustType::Isize,
                    "u8" => RustType::U8,
                    "u16" => RustType::U16,
                    "u32" => RustType::U32,
                    "u64" => RustType::U64,
                    "u128" => RustType::U128,
                    "usize" => RustType::Usize,
                    "f32" => RustType::F32,
                    "f64" => RustType::F64,
                    _ => RustType::Infer,
                }
            }
            "type_identifier" | "scoped_type_identifier" => {
                let name = source[node.start_byte()..node.end_byte()].to_string();
                RustType::Named {
                    name,
                    module: None,
                    type_args: vec![],
                    lifetime_args: vec![],
                }
            }
            "reference_type" => {
                let mutable = node.child_by_field_name("mutable_specifier").is_some();
                let inner = node
                    .child_by_field_name("type")
                    .map(|n| self.parse_type(&n, source))
                    .unwrap_or(RustType::Infer);
                RustType::Reference {
                    lifetime: None,
                    mutable,
                    inner: Box::new(inner),
                }
            }
            "tuple_type" => {
                let mut elements = Vec::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if !matches!(child.kind(), "(" | ")" | ",") {
                        elements.push(self.parse_type(&child, source));
                    }
                }
                RustType::Tuple(elements)
            }
            "array_type" => {
                let element = node
                    .child_by_field_name("element")
                    .map(|n| self.parse_type(&n, source))
                    .unwrap_or(RustType::Infer);
                RustType::Array {
                    element: Box::new(element),
                    size: 0,
                }
            }
            "unit_type" => RustType::Unit,
            "never_type" => RustType::Never,
            _ => RustType::Infer,
        }
    }
}
