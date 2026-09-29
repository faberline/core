use tree_sitter::Node;

use crate::domain::rust_type_system::infer::{
    RustTypeError, RustTypeErrorKind, RustTypeInferencer,
};
use crate::domain::rust_type_system::types::{RustType, StructFields};

impl RustTypeInferencer {
    /// Infer integer literal type
    pub(super) fn infer_integer_literal(&mut self, node: &Node, source: &str) -> RustType {
        let text = &source[node.start_byte()..node.end_byte()];

        // Check for type suffix
        if text.ends_with("i8") {
            RustType::I8
        } else if text.ends_with("i16") {
            RustType::I16
        } else if text.ends_with("i32") {
            RustType::I32
        } else if text.ends_with("i64") {
            RustType::I64
        } else if text.ends_with("i128") {
            RustType::I128
        } else if text.ends_with("isize") {
            RustType::Isize
        } else if text.ends_with("u8") {
            RustType::U8
        } else if text.ends_with("u16") {
            RustType::U16
        } else if text.ends_with("u32") {
            RustType::U32
        } else if text.ends_with("u64") {
            RustType::U64
        } else if text.ends_with("u128") {
            RustType::U128
        } else if text.ends_with("usize") {
            RustType::Usize
        } else {
            // Default to i32 (will be refined by context)
            RustType::I32
        }
    }

    /// Infer identifier type from context
    pub(super) fn infer_identifier(&mut self, node: &Node, source: &str) -> RustType {
        let name = &source[node.start_byte()..node.end_byte()];

        if let Some(ty) = self.context.lookup_type(name) {
            ty
        } else {
            self.errors.push(RustTypeError {
                message: format!("Unbound identifier: {}", name),
                span: Some((node.start_byte(), node.end_byte())),
                kind: RustTypeErrorKind::UnboundTypeVar,
            });
            RustType::Error
        }
    }

    /// Infer call expression type
    pub(super) fn infer_call_expr(&mut self, node: &Node, source: &str) -> RustType {
        if let Some(func) = node.child_by_field_name("function") {
            // Check if this is a method call (field_expression followed by call)
            if func.kind() == "field_expression" {
                return self.infer_method_call(&func, node, source);
            }

            let func_type = self.infer_expr(&func, source);

            match &func_type {
                RustType::FnPointer { return_type, .. } => *return_type.clone(),
                RustType::Closure { return_type, .. } => *return_type.clone(),
                RustType::Named { .. } => {
                    // Constructor call - return the named type
                    func_type.clone()
                }
                _ => {
                    // Try to resolve as method call
                    self.context.fresh_type_var("call_result")
                }
            }
        } else {
            RustType::Infer
        }
    }

    /// Infer method call type using trait resolver
    fn infer_method_call(
        &mut self,
        field_expr: &Node,
        _call_node: &Node,
        source: &str,
    ) -> RustType {
        if let (Some(value), Some(field)) = (
            field_expr.child_by_field_name("value"),
            field_expr.child_by_field_name("field"),
        ) {
            let receiver_type = self.infer_expr(&value, source);
            let method_name = &source[field.start_byte()..field.end_byte()];

            // Try to resolve the method using the trait resolver
            if let Some(resolution) = self
                .trait_resolver
                .resolve_method(&receiver_type, method_name)
            {
                return resolution.method.signature.return_type.clone();
            }

            // Fallback: check if it's a field that's callable
            let field_type = self.resolve_field_type(&receiver_type, method_name);
            match &field_type {
                RustType::FnPointer { return_type, .. } => return *return_type.clone(),
                RustType::Closure { return_type, .. } => return *return_type.clone(),
                _ => {}
            }

            // Method not found - generate a fresh type var
            self.context.fresh_type_var("method_result")
        } else {
            RustType::Infer
        }
    }

    /// Infer field expression type
    pub(super) fn infer_field_expr(&mut self, node: &Node, source: &str) -> RustType {
        if let (Some(value), Some(field)) = (
            node.child_by_field_name("value"),
            node.child_by_field_name("field"),
        ) {
            let base_type = self.infer_expr(&value, source);
            let field_name = &source[field.start_byte()..field.end_byte()];

            self.resolve_field_type(&base_type, field_name)
        } else {
            RustType::Infer
        }
    }

    /// Resolve field type from a struct/enum type
    fn resolve_field_type(&self, base_type: &RustType, field_name: &str) -> RustType {
        match base_type {
            RustType::Named { name, .. } => {
                if let Some(struct_def) = self.context.struct_defs.get(name) {
                    if let StructFields::Named(fields) = &struct_def.fields {
                        for field in fields {
                            if field.name == field_name {
                                return field.ty.clone();
                            }
                        }
                    }
                }
                RustType::Infer
            }
            RustType::Reference { inner, .. } => {
                // Auto-deref
                self.resolve_field_type(inner, field_name)
            }
            RustType::Tuple(elements) => {
                // Tuple field access (e.g., tuple.0)
                if let Ok(index) = field_name.parse::<usize>() {
                    elements.get(index).cloned().unwrap_or(RustType::Error)
                } else {
                    RustType::Error
                }
            }
            _ => RustType::Infer,
        }
    }

    /// Infer index expression type
    pub(super) fn infer_index_expr(&mut self, node: &Node, source: &str) -> RustType {
        if let Some(value) = node.child_by_field_name("value") {
            let base_type = self.infer_expr(&value, source);

            match &base_type {
                RustType::Array { element, .. } => *element.clone(),
                RustType::Slice(element) => *element.clone(),
                RustType::Reference { inner, .. } => {
                    // Auto-deref for slices/arrays
                    match inner.as_ref() {
                        RustType::Array { element, .. } => *element.clone(),
                        RustType::Slice(element) => *element.clone(),
                        _ => RustType::Infer,
                    }
                }
                _ => RustType::Infer,
            }
        } else {
            RustType::Infer
        }
    }

    /// Infer reference expression type
    pub(super) fn infer_reference_expr(&mut self, node: &Node, source: &str) -> RustType {
        let mutable = node.child_by_field_name("mutable_specifier").is_some();

        if let Some(value) = node.child_by_field_name("value") {
            let inner_type = self.infer_expr(&value, source);
            RustType::Reference {
                lifetime: Some(self.context.fresh_lifetime()),
                mutable,
                inner: Box::new(inner_type),
            }
        } else {
            RustType::Infer
        }
    }

    /// Infer dereference expression type
    pub(super) fn infer_deref_expr(&mut self, node: &Node, source: &str) -> RustType {
        if let Some(value) = node.child_by_field_name("value") {
            let ptr_type = self.infer_expr(&value, source);

            match ptr_type {
                RustType::Reference { inner, .. } => *inner,
                RustType::RawPointer { inner, .. } => *inner,
                _ => {
                    // Try Deref trait
                    RustType::Infer
                }
            }
        } else {
            RustType::Infer
        }
    }
}
