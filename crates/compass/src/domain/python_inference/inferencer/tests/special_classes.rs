use super::*;

// ============= Phase G Tests =============

#[test]
fn test_dataclass_analysis() {
    let code = r#"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int
    label: str = "default"
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    // Analyze all nodes (handling decorated_definition wrapper)
    let root = parsed.tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "decorated_definition" {
            // The decorated class is wrapped in decorated_definition
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "class_definition" {
                    inferencer.analyze_class(&inner);
                }
            }
        } else if child.kind() == "class_definition" {
            inferencer.analyze_class(&child);
        }
    }

    // Check that Point class was registered
    let class_info = inferencer.get_class("Point");
    assert!(class_info.is_some(), "Point class should be registered");
    let class_info = class_info.unwrap();

    // Check that __init__ was generated
    assert!(
        class_info.methods.contains_key("__init__"),
        "Dataclass should have __init__ method"
    );

    // Check that fields are registered as attributes
    assert!(
        class_info.attributes.contains_key("x"),
        "x attribute should exist"
    );
    assert!(
        class_info.attributes.contains_key("y"),
        "y attribute should exist"
    );
    assert!(
        class_info.attributes.contains_key("label"),
        "label attribute should exist"
    );
}

#[test]
fn test_namedtuple_analysis() {
    let code = r#"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    // Analyze all nodes
    let root = parsed.tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "class_definition" {
            inferencer.analyze_class(&child);
        }
    }

    // Check that Point class was registered
    let class_info = inferencer.get_class("Point");
    assert!(class_info.is_some(), "Point class should be registered");
    let class_info = class_info.unwrap();

    // Check that fields are registered as attributes
    assert!(
        class_info.attributes.contains_key("x"),
        "x attribute should exist on NamedTuple"
    );
    assert!(
        class_info.attributes.contains_key("y"),
        "y attribute should exist on NamedTuple"
    );
}

#[test]
fn test_property_analysis() {
    let code = r#"
class Circle:
    def __init__(self, radius: float) -> None:
        self._radius = radius

    @property
    def radius(self) -> float:
        return self._radius

    @property
    def area(self) -> float:
        return 3.14159 * self._radius ** 2

    def normal_method(self) -> str:
        return "not a property"
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    // Analyze all nodes
    let root = parsed.tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "class_definition" {
            inferencer.analyze_class(&child);
        }
    }

    // Check that Circle class was registered
    let class_info = inferencer.get_class("Circle");
    assert!(class_info.is_some(), "Circle class should be registered");
    let class_info = class_info.unwrap();

    // Properties should be registered as attributes, not methods
    assert!(
        class_info.attributes.contains_key("radius"),
        "radius should be an attribute (property)"
    );
    assert!(
        class_info.attributes.contains_key("area"),
        "area should be an attribute (property)"
    );

    // Normal method should still be a method
    assert!(
        class_info.methods.contains_key("normal_method"),
        "normal_method should be a method"
    );
    assert!(
        !class_info.attributes.contains_key("normal_method"),
        "normal_method should NOT be an attribute"
    );

    // Check that properties have correct types (from return annotation)
    let radius_type = class_info.attributes.get("radius");
    assert!(radius_type.is_some());
    assert_eq!(*radius_type.unwrap(), Type::Float);

    let area_type = class_info.attributes.get("area");
    assert!(area_type.is_some());
    assert_eq!(*area_type.unwrap(), Type::Float);
}
