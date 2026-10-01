use tree_sitter::Node;

use crate::domain::python_inference::inferencer::TypeInferencer;
use crate::domain::type_system::annotation::parse_type_annotation;
use crate::domain::type_system::class_info::ClassInfo;
use crate::domain::type_system::ty::{Param, ParamKind, Type};

impl<'a> TypeInferencer<'a> {
    /// Check if a function has the @overload decorator
    pub(super) fn has_overload_decorator(&self, node: &Node) -> bool {
        // Look for decorator node
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "decorator" {
                let decorator_text = self.node_text(&child);
                if decorator_text.contains("overload") {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a class has the @dataclass decorator
    /// Looks in both the node itself and its parent (for decorated_definition wrapper)
    pub(super) fn has_dataclass_decorator(&self, node: &Node) -> bool {
        // First check the node's children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "decorator" {
                let decorator_text = self.node_text(&child);
                if decorator_text.contains("dataclass") {
                    return true;
                }
            }
        }

        // Also check the parent if it's a decorated_definition
        if let Some(parent) = node.parent() {
            if parent.kind() == "decorated_definition" {
                let mut cursor = parent.walk();
                for child in parent.children(&mut cursor) {
                    if child.kind() == "decorator" {
                        let decorator_text = self.node_text(&child);
                        if decorator_text.contains("dataclass") {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Analyze a dataclass and generate __init__ signature from fields
    pub fn analyze_dataclass(&mut self, node: &Node) -> Type {
        let class_name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_default();

        let mut fields: Vec<(String, Type, bool)> = Vec::new(); // (name, type, has_default)

        // Find the class body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                match child.kind() {
                    // Annotated assignment: field_name: Type or field_name: Type = default
                    "expression_statement" => {
                        if let Some(assignment) = child.child(0) {
                            if assignment.kind() == "assignment" {
                                if let Some((name, ty, has_default)) =
                                    self.parse_dataclass_field(&assignment)
                                {
                                    fields.push((name, ty, has_default));
                                }
                            }
                        }
                    }
                    // Type annotated field: field_name: Type
                    "type" => {
                        if let Some(name) = child.child_by_field_name("name") {
                            let field_name = self.node_text(&name).to_string();
                            let field_type = child
                                .child_by_field_name("type")
                                .map(|t| parse_type_annotation(self.source, &t))
                                .unwrap_or(Type::Any);
                            let has_default = child.child_by_field_name("value").is_some();
                            fields.push((field_name, field_type, has_default));
                        }
                    }
                    _ => {}
                }
            }
        }

        // Generate __init__ method from fields
        let init_params: Vec<Param> = fields
            .iter()
            .map(|(name, ty, has_default)| Param {
                name: name.clone(),
                ty: ty.clone(),
                has_default: *has_default,
                kind: ParamKind::Positional,
            })
            .collect();

        let init_type = Type::Callable {
            params: init_params,
            ret: Box::new(Type::None),
        };

        // Register the class with __init__
        let mut class_info = ClassInfo::new(class_name.clone());
        class_info.methods.insert("__init__".to_string(), init_type);

        // Add field types as attributes
        for (name, ty, _) in &fields {
            class_info.attributes.insert(name.clone(), ty.clone());
        }

        self.classes.insert(class_name.clone(), class_info);

        Type::ClassType {
            name: class_name,
            module: None,
        }
    }

    /// Parse a dataclass field from an assignment node
    fn parse_dataclass_field(&self, node: &Node) -> Option<(String, Type, bool)> {
        // Look for type-annotated assignment: name: Type = value or name: Type
        let left = node.child_by_field_name("left")?;

        if left.kind() == "identifier" {
            let name = self.node_text(&left).to_string();

            // Check for type annotation
            let annotation = node.child_by_field_name("type");
            let ty = annotation
                .map(|a| parse_type_annotation(self.source, &a))
                .unwrap_or(Type::Any);

            let has_default = node.child_by_field_name("right").is_some();

            return Some((name, ty, has_default));
        }

        None
    }

    /// Check if this is a NamedTuple class definition
    pub(super) fn is_namedtuple_class(&self, node: &Node) -> bool {
        // Check for class Foo(NamedTuple):
        // tree-sitter-python uses argument_list for base classes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "argument_list" {
                let bases_text = self.node_text(&child);
                if bases_text.contains("NamedTuple") {
                    return true;
                }
            }
        }
        false
    }

    /// Analyze a NamedTuple class and generate proper types
    pub fn analyze_namedtuple(&mut self, node: &Node) -> Type {
        let class_name = node
            .child_by_field_name("name")
            .map(|n| self.node_text(&n).to_string())
            .unwrap_or_default();

        let mut field_types: Vec<Type> = Vec::new();
        let mut fields: Vec<(String, Type)> = Vec::new();

        // Find the class body and extract field annotations
        // tree-sitter parses `x: int` as an assignment node with a type child
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "expression_statement" {
                    if let Some(assignment) = child.child(0) {
                        if assignment.kind() == "assignment" {
                            // Get the field name from the left (identifier)
                            if let Some(name_node) = assignment.child(0) {
                                if name_node.kind() == "identifier" {
                                    let field_name = self.node_text(&name_node).to_string();

                                    // Get the type from the type annotation
                                    let mut type_cursor = assignment.walk();
                                    let mut field_type = Type::Any;
                                    for assign_child in assignment.children(&mut type_cursor) {
                                        if assign_child.kind() == "type" {
                                            field_type =
                                                parse_type_annotation(self.source, &assign_child);
                                            break;
                                        }
                                    }

                                    fields.push((field_name, field_type.clone()));
                                    field_types.push(field_type);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Create a tuple type for the NamedTuple (unused for now but may be useful)
        let _tuple_type = Type::Tuple(field_types);

        // Register class with field attributes
        let mut class_info = ClassInfo::new(class_name.clone());
        for (name, ty) in &fields {
            class_info.attributes.insert(name.clone(), ty.clone());
        }
        self.classes.insert(class_name.clone(), class_info);

        // Bind the class name to a ClassType
        let class_type = Type::ClassType {
            name: class_name.clone(),
            module: None,
        };
        self.env.bind(class_name, class_type.clone());

        class_type
    }

    /// Check if a class or method has @property decorator
    /// Looks in both the node itself and its parent (for decorated_definition wrapper)
    pub fn has_property_decorator(&self, node: &Node) -> bool {
        // First check the node's children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "decorator" {
                let decorator_text = self.node_text(&child);
                if decorator_text.contains("property") {
                    return true;
                }
            }
        }

        // Also check the parent if it's a decorated_definition
        if let Some(parent) = node.parent() {
            if parent.kind() == "decorated_definition" {
                let mut cursor = parent.walk();
                for child in parent.children(&mut cursor) {
                    if child.kind() == "decorator" {
                        let decorator_text = self.node_text(&child);
                        if decorator_text.contains("property") {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Get property type (return type of getter)
    pub fn get_property_type(&mut self, node: &Node) -> Type {
        // Property type is the return type of the getter method
        node.child_by_field_name("return_type")
            .map(|n| parse_type_annotation(self.source, &n))
            .unwrap_or(Type::Unknown)
    }
}
