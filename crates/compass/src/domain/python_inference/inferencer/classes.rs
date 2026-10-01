use tree_sitter::Node;

use crate::domain::python_inference::inferencer::TypeInferencer;
use crate::domain::type_system::annotation::parse_type_annotation;
use crate::domain::type_system::class_info::ClassInfo;
use crate::domain::type_system::ty::Type;

impl<'a> TypeInferencer<'a> {
    /// Analyze a class definition and add it to the registry
    pub fn analyze_class(&mut self, node: &Node) -> ClassInfo {
        // Check for special class types first
        if self.has_dataclass_decorator(node) {
            self.analyze_dataclass(node);
            // Get the class name and return the info
            let name = node
                .child_by_field_name("name")
                .map(|n| self.node_text(&n).to_string())
                .unwrap_or_default();
            return self.classes.get(&name).cloned().unwrap_or_default();
        }

        if self.is_namedtuple_class(node) {
            self.analyze_namedtuple(node);
            let name = node
                .child_by_field_name("name")
                .map(|n| self.node_text(&n).to_string())
                .unwrap_or_default();
            return self.classes.get(&name).cloned().unwrap_or_default();
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_default();

        // Set current class for Self type resolution
        let prev_class = self.current_class.take();
        self.current_class = Some(name.clone());

        let mut class_info = ClassInfo::new(name.clone());

        // Parse base classes
        if let Some(bases) = node.child_by_field_name("superclasses") {
            let mut cursor = bases.walk();
            for child in bases.children(&mut cursor) {
                if child.kind() == "identifier" || child.kind() == "attribute" {
                    class_info.bases.push(self.node_text(&child).to_string());
                }
            }
        }

        // Parse class body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                match child.kind() {
                    "function_definition" | "async_function_definition" => {
                        self.parse_class_method(&child, &mut class_info);
                    }
                    "decorated_definition" => {
                        // Handle decorated methods (e.g., @property, @staticmethod)
                        let mut inner_cursor = child.walk();
                        for inner in child.children(&mut inner_cursor) {
                            if inner.kind() == "function_definition"
                                || inner.kind() == "async_function_definition"
                            {
                                self.parse_class_method(&inner, &mut class_info);
                            }
                        }
                    }
                    "expression_statement" => {
                        self.parse_class_attribute(&child, &mut class_info);
                    }
                    _ => {}
                }
            }
        }

        // Restore previous class context
        self.current_class = prev_class;

        // Add class type to environment
        let class_type = Type::ClassType {
            name: name.clone(),
            module: None,
        };
        self.env.bind(name.clone(), class_type);

        // Store class info
        self.classes.insert(name, class_info.clone());

        class_info
    }

    /// Parse a method in a class definition
    fn parse_class_method(&mut self, node: &Node, class_info: &mut ClassInfo) {
        let method_name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_default();

        // Check for @property decorator - properties become attributes, not methods
        if self.has_property_decorator(node) {
            let property_type = self.get_property_type(node);
            class_info.attributes.insert(method_name, property_type);
            return;
        }

        let mut params = Vec::new();
        let mut return_type = Type::Unknown;

        // Parse parameters
        if let Some(params_node) = node.child_by_field_name("parameters") {
            params = self.parse_parameters(&params_node);

            // Check for self parameter and extract attribute assignments
            if let Some(first_param) = params.first() {
                if first_param.name == "self" {
                    // This is an instance method
                    // Parse body for self.attr = ... assignments
                    if method_name == "__init__" {
                        if let Some(body) = node.child_by_field_name("body") {
                            self.parse_init_assignments(&body, class_info);
                        }
                    }
                }
            }
        }

        // Parse return type
        if let Some(return_node) = node.child_by_field_name("return_type") {
            return_type = parse_type_annotation(self.source, &return_node);
        }

        // Handle special return type for __init__
        if method_name == "__init__" {
            return_type = Type::None;
        }

        let method_type = Type::Callable {
            params,
            ret: Box::new(return_type),
        };

        class_info.methods.insert(method_name, method_type);
    }

    /// Parse attribute assignments in __init__
    fn parse_init_assignments(&mut self, body: &Node, class_info: &mut ClassInfo) {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "expression_statement" {
                if let Some(expr) = child.child(0) {
                    if expr.kind() == "assignment" {
                        self.parse_self_assignment(&expr, class_info);
                    }
                }
            }
        }
    }

    /// Parse self.attr = value assignments
    fn parse_self_assignment(&mut self, node: &Node, class_info: &mut ClassInfo) {
        if let Some(left) = node.child_by_field_name("left") {
            if left.kind() == "attribute" {
                if let Some(obj) = left.child_by_field_name("object") {
                    if self.node_text(&obj) == "self" {
                        if let Some(attr) = left.child_by_field_name("attribute") {
                            let attr_name = self.node_text(&attr).to_string();

                            // Get type from annotation or infer from value
                            let attr_type =
                                if let Some(type_node) = node.child_by_field_name("type") {
                                    parse_type_annotation(self.source, &type_node)
                                } else if let Some(value) = node.child_by_field_name("right") {
                                    self.infer_expr(&value)
                                } else {
                                    Type::Unknown
                                };

                            class_info.attributes.insert(attr_name, attr_type);
                        }
                    }
                }
            }
        }
    }

    /// Parse class-level attribute (class variable or annotated attribute)
    fn parse_class_attribute(&mut self, node: &Node, class_info: &mut ClassInfo) {
        if let Some(expr) = node.child(0) {
            match expr.kind() {
                "assignment" => {
                    // name = value or name: type = value
                    if let Some(left) = expr.child_by_field_name("left") {
                        if left.kind() == "identifier" {
                            let attr_name = self.node_text(&left).to_string();
                            let attr_type =
                                if let Some(type_node) = expr.child_by_field_name("type") {
                                    parse_type_annotation(self.source, &type_node)
                                } else if let Some(value) = expr.child_by_field_name("right") {
                                    self.infer_expr(&value)
                                } else {
                                    Type::Unknown
                                };
                            class_info.class_vars.insert(attr_name, attr_type);
                        }
                    }
                }
                // Annotated attribute without assignment: name: type
                _ => {}
            }
        }
    }
}
