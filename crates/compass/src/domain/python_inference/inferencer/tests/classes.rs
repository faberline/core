use super::*;

#[test]
fn test_class_analysis() {
    let code = r#"
class Person:
    name: str
    age: int = 0

    def __init__(self, name: str, age: int) -> None:
        self.name = name
        self.age = age

    def greet(self) -> str:
        return "Hello, " + self.name
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    // Find class definition
    let root = parsed.tree.root_node();
    if let Some(class_node) = root.child(0) {
        if class_node.kind() == "class_definition" {
            let class_info = inferencer.analyze_class(&class_node);

            assert_eq!(class_info.name, "Person");

            // Check class variables
            assert!(class_info.class_vars.contains_key("name"));
            assert!(class_info.class_vars.contains_key("age"));

            // Check methods
            assert!(class_info.methods.contains_key("__init__"));
            assert!(class_info.methods.contains_key("greet"));

            // Check __init__ sets instance attributes
            assert!(class_info.attributes.contains_key("name"));
            assert!(class_info.attributes.contains_key("age"));
        }
    }
}

#[test]
fn test_class_attribute_inference() {
    let code = r#"
class Point:
    def __init__(self, x: int, y: int) -> None:
        self.x = x
        self.y = y

p = Point(1, 2)
p.x
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    // Walk through the code to analyze class and assignments
    let root = parsed.tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "class_definition" {
            inferencer.analyze_class(&child);
        }
    }

    // Check that Point class was registered
    let class_info = inferencer.get_class("Point");
    assert!(class_info.is_some());
    let class_info = class_info.unwrap();
    assert!(class_info.attributes.contains_key("x"));
    assert!(class_info.attributes.contains_key("y"));
}

#[test]
fn test_inheritance_attribute_lookup() {
    let code = r#"
class Animal:
    species: str = "unknown"

    def speak(self) -> str:
        return "sound"

class Dog(Animal):
    def bark(self) -> str:
        return "woof"
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    // Analyze all classes
    let root = parsed.tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "class_definition" {
            inferencer.analyze_class(&child);
        }
    }

    // Dog should have its own method
    let bark = inferencer.get_attribute_recursive("Dog", "bark");
    assert!(bark.is_some());

    // Dog should inherit speak from Animal
    let speak = inferencer.get_attribute_recursive("Dog", "speak");
    assert!(speak.is_some());

    // Dog should inherit class var from Animal
    let species = inferencer.get_attribute_recursive("Dog", "species");
    assert!(species.is_some());

    // Animal should not have bark
    let animal_bark = inferencer.get_attribute_recursive("Animal", "bark");
    assert!(animal_bark.is_none());
}

#[test]
fn test_is_subclass() {
    let code = r#"
class Animal:
    pass

class Dog(Animal):
    pass

class Labrador(Dog):
    pass
"#;
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    // Analyze all classes
    let root = parsed.tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "class_definition" {
            inferencer.analyze_class(&child);
        }
    }

    // Self is a subclass of self
    assert!(inferencer.is_subclass("Dog", "Dog"));

    // Dog is a subclass of Animal
    assert!(inferencer.is_subclass("Dog", "Animal"));

    // Labrador is a subclass of Dog and Animal (transitive)
    assert!(inferencer.is_subclass("Labrador", "Dog"));
    assert!(inferencer.is_subclass("Labrador", "Animal"));

    // Animal is NOT a subclass of Dog
    assert!(!inferencer.is_subclass("Animal", "Dog"));
}
