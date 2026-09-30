use crate::domain::syntax::parsed_file::NodeRange;
use std::collections::HashMap;

use tree_sitter::Node;

use crate::diagnostic::{Diagnostic, DiagnosticCategory, Range};
use crate::domain::type_checking::checker::TypeChecker;
use crate::domain::type_system::ty::{Param, ParamKind, Type};

impl<'a> TypeChecker<'a> {
    /// Check function call
    pub(super) fn check_call(&mut self, node: &Node) {
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return,
        };

        let func_type = self.inferencer.infer_expr(&func);

        match &func_type {
            Type::Callable { params, .. } => {
                self.check_call_arguments(node, params);
            }
            Type::Unknown | Type::Any => {
                // Can't check unknown/any types
            }
            _ => {
                // Not callable
                self.diagnostics.push(Diagnostic::error(
                    func.to_range(),
                    "TC004",
                    DiagnosticCategory::Type,
                    format!("Type '{}' is not callable", func_type),
                ));
            }
        }
    }

    /// Check call arguments
    fn check_call_arguments(&mut self, call: &Node, params: &[Param]) {
        let args = match call.child_by_field_name("arguments") {
            Some(a) => a,
            None => return,
        };

        let mut positional_args: Vec<(Range, Type)> = Vec::new();
        let mut keyword_args: HashMap<String, (Range, Type)> = HashMap::new();
        let mut cursor = args.walk();

        for child in args.children(&mut cursor) {
            match child.kind() {
                "(" | ")" | "," => continue,
                "keyword_argument" => {
                    // Parse keyword argument: name=value
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = self.node_text(&name_node).to_string();
                        if let Some(value_node) = child.child_by_field_name("value") {
                            let value_type = self.inferencer.infer_expr(&value_node);
                            keyword_args.insert(name, (child.to_range(), value_type));
                        }
                    }
                }
                _ => {
                    positional_args.push((child.to_range(), self.inferencer.infer_expr(&child)));
                }
            }
        }

        // Build param lookup by name
        let param_by_name: HashMap<&str, &Param> =
            params.iter().map(|p| (p.name.as_str(), p)).collect();

        // Check for too many positional arguments
        let max_positional = params
            .iter()
            .filter(|p| !matches!(p.kind, ParamKind::VarPositional | ParamKind::VarKeyword))
            .count();

        if positional_args.len() > max_positional
            && !params
                .iter()
                .any(|p| matches!(p.kind, ParamKind::VarPositional))
        {
            self.diagnostics.push(Diagnostic::error(
                args.to_range(),
                "TC006",
                DiagnosticCategory::Type,
                format!(
                    "Too many arguments: expected at most {}, got {}",
                    max_positional,
                    positional_args.len()
                ),
            ));
        }

        // Check positional argument types
        for (i, (range, arg_ty)) in positional_args.iter().enumerate() {
            if let Some(param) = params.get(i) {
                if !param.ty.is_unknown() && !param.ty.is_any() {
                    if !self.is_assignable(&param.ty, arg_ty) {
                        self.diagnostics.push(Diagnostic::error(
                            range.clone(),
                            "TC005",
                            DiagnosticCategory::Type,
                            format!(
                                "Argument '{}' type mismatch: expected '{}', got '{}'",
                                param.name, param.ty, arg_ty
                            ),
                        ));
                    }
                }
            }
        }

        // Check keyword argument types
        for (name, (range, arg_ty)) in &keyword_args {
            if let Some(param) = param_by_name.get(name.as_str()) {
                if !param.ty.is_unknown() && !param.ty.is_any() {
                    if !self.is_assignable(&param.ty, arg_ty) {
                        self.diagnostics.push(Diagnostic::error(
                            range.clone(),
                            "TC005",
                            DiagnosticCategory::Type,
                            format!(
                                "Argument '{}' type mismatch: expected '{}', got '{}'",
                                name, param.ty, arg_ty
                            ),
                        ));
                    }
                }
            } else if !params
                .iter()
                .any(|p| matches!(p.kind, ParamKind::VarKeyword))
            {
                // Unknown keyword argument
                self.diagnostics.push(Diagnostic::error(
                    range.clone(),
                    "TC008",
                    DiagnosticCategory::Type,
                    format!("Unknown keyword argument: '{}'", name),
                ));
            }
        }

        // Check for missing required arguments
        let provided_count = positional_args.len();
        for (i, param) in params.iter().enumerate() {
            if !param.has_default
                && !matches!(param.kind, ParamKind::VarPositional | ParamKind::VarKeyword)
            {
                let provided_positionally = i < provided_count;
                let provided_by_keyword = keyword_args.contains_key(&param.name);

                if !provided_positionally && !provided_by_keyword {
                    self.diagnostics.push(Diagnostic::error(
                        args.to_range(),
                        "TC007",
                        DiagnosticCategory::Type,
                        format!("Missing required argument: '{}'", param.name),
                    ));
                }
            }
        }
    }

    /// Check attribute access
    pub(super) fn check_attribute(&mut self, node: &Node) {
        let object = match node.child_by_field_name("object") {
            Some(o) => o,
            None => return,
        };

        let object_type = self.inferencer.infer_expr(&object);

        // Check for None attribute access
        if object_type.contains_none() {
            let attr = node
                .child_by_field_name("attribute")
                .map(|a| self.node_text(&a))
                .unwrap_or("?");

            self.diagnostics.push(Diagnostic::warning(
                node.to_range(),
                "TC009",
                DiagnosticCategory::Type,
                format!(
                    "Accessing '{}' on potentially None value (type: {})",
                    attr, object_type
                ),
            ));
        }
    }
}
