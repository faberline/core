use std::collections::HashMap;

use tree_sitter::Node;

use crate::domain::python_inference::inferencer::TypeInferencer;
use crate::domain::type_system::ty::{Param, ParamKind, Type};

impl<'a> TypeInferencer<'a> {
    /// Infer function call result type with generic type inference
    pub(super) fn infer_call(&mut self, node: &Node) -> Type {
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return Type::Unknown,
        };

        // Check if this is a method call on an instance (e.g., user.save())
        if func.kind() == "attribute" {
            let object = func.child_by_field_name("object");
            let attribute = func.child_by_field_name("attribute");

            if let (Some(obj), Some(attr)) = (object, attribute) {
                let object_type = self.infer_expr(&obj);
                let method_name = self.node_text(&attr);

                // Try framework-specific method signature
                if let Some(method_sig) = self
                    .framework_registry
                    .get_method_signature(&object_type, method_name)
                {
                    return method_sig.return_type;
                }
            }
        }

        let func_ty = self.infer_expr(&func);

        match &func_ty {
            Type::Callable { params, ret } => {
                // Collect argument types
                let arg_types = self.collect_call_arguments(node);

                // Check if return type has type variables
                let type_vars = ret.type_vars();
                if type_vars.is_empty() {
                    // No generics, just return the declared return type
                    return (**ret).clone();
                }

                // Unify parameter types with argument types to infer TypeVars
                let mut substitutions = HashMap::new();
                for (param, arg_ty) in params.iter().zip(arg_types.iter()) {
                    param.ty.unify(arg_ty, &mut substitutions);
                }

                // Apply substitutions to the return type
                ret.substitute(&substitutions)
            }
            Type::Instance { name, .. } => {
                // Constructor call returns instance
                Type::Instance {
                    name: name.clone(),
                    module: None,
                    type_args: vec![],
                }
            }
            Type::ClassType { name, .. } => {
                // type[T]() returns T
                Type::Instance {
                    name: name.clone(),
                    module: None,
                    type_args: vec![],
                }
            }
            _ => Type::Unknown,
        }
    }

    /// Collect argument types from a call expression
    fn collect_call_arguments(&mut self, node: &Node) -> Vec<Type> {
        let mut arg_types = Vec::new();

        if let Some(args_node) = node.child_by_field_name("arguments") {
            let mut cursor = args_node.walk();
            for child in args_node.children(&mut cursor) {
                match child.kind() {
                    "(" | ")" | "," => continue,
                    "keyword_argument" => {
                        // Skip keyword for now, just get the value
                        if let Some(value) = child.child_by_field_name("value") {
                            arg_types.push(self.infer_expr(&value));
                        }
                    }
                    _ => {
                        arg_types.push(self.infer_expr(&child));
                    }
                }
            }
        }

        arg_types
    }

    /// Infer attribute access type
    pub(super) fn infer_attribute(&mut self, node: &Node) -> Type {
        let object = match node.child_by_field_name("object") {
            Some(o) => o,
            None => return Type::Unknown,
        };
        let attribute = match node.child_by_field_name("attribute") {
            Some(a) => a,
            None => return Type::Unknown,
        };

        let object_type = self.infer_expr(&object);
        let attr_name = self.node_text(&attribute);

        match &object_type {
            Type::Instance { name, .. } => {
                // 1. Try standard attribute resolution with inheritance
                if let Some(ty) = self.get_attribute_recursive(name, attr_name) {
                    return ty;
                }

                // 2. Try framework-specific attribute resolution
                if let Some(ty) = self
                    .framework_registry
                    .get_attribute_type(&object_type, attr_name)
                {
                    return ty;
                }

                Type::Unknown
            }
            Type::ClassType { name, .. } => {
                // Class attribute access (static methods, class vars) with inheritance
                self.get_class_attribute_recursive(name, attr_name)
                    .unwrap_or(Type::Unknown)
            }
            Type::Optional(inner) => {
                // For Optional[T].attr, return the attribute type from T with inheritance
                if let Type::Instance { name, .. } = inner.as_ref() {
                    if let Some(ty) = self.get_attribute_recursive(name, attr_name) {
                        return ty;
                    }

                    // Try framework-specific attribute resolution
                    if let Some(ty) = self
                        .framework_registry
                        .get_attribute_type(inner.as_ref(), attr_name)
                    {
                        return ty;
                    }
                }
                Type::Unknown
            }
            _ => Type::Unknown,
        }
    }

    /// Infer subscript type
    pub(super) fn infer_subscript(&mut self, node: &Node) -> Type {
        let value = match node.child_by_field_name("value") {
            Some(v) => v,
            None => return Type::Unknown,
        };

        let value_ty = self.infer_expr(&value);

        match &value_ty {
            Type::List(elem) => (**elem).clone(),
            Type::Dict(_, val) => (**val).clone(),
            Type::Tuple(elems) => {
                // For tuple, try to get specific index
                if let Some(subscript) = node.child_by_field_name("subscript") {
                    if subscript.kind() == "integer" {
                        if let Ok(idx) = self.node_text(&subscript).parse::<usize>() {
                            if let Some(ty) = elems.get(idx) {
                                return ty.clone();
                            }
                        }
                    }
                }
                // Unknown index, return union of all element types
                Type::union(elems.clone())
            }
            Type::Str => Type::Str, // str[n] -> str
            _ => Type::Unknown,
        }
    }

    /// Infer conditional expression type
    pub(super) fn infer_conditional(&mut self, node: &Node) -> Type {
        // Python: true_val if condition else false_val
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();

        // [true_val, "if", condition, "else", false_val]
        if children.len() >= 5 {
            let true_ty = self.infer_expr(&children[0]);
            let false_ty = self.infer_expr(&children[4]);

            if true_ty == false_ty {
                true_ty
            } else {
                Type::union(vec![true_ty, false_ty])
            }
        } else {
            Type::Unknown
        }
    }

    /// Infer lambda type
    pub(super) fn infer_lambda(&mut self, node: &Node) -> Type {
        // lambda params: body
        let mut params = Vec::new();

        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    params.push(Param {
                        name: self.node_text(&child).to_string(),
                        ty: Type::Unknown,
                        has_default: false,
                        kind: ParamKind::Positional,
                    });
                }
            }
        }

        let ret = if let Some(body) = node.child_by_field_name("body") {
            self.infer_expr(&body)
        } else {
            Type::Unknown
        };

        Type::Callable {
            params,
            ret: Box::new(ret),
        }
    }
}
