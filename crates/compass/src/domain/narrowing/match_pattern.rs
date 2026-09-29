use crate::domain::narrowing::condition::NarrowingCondition;
use crate::domain::narrowing::condition_parser::parse_simple_type_name;
use crate::domain::type_system::ty::Type;

/// Represents a match case pattern for type narrowing (PEP 634)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum MatchPattern {
    /// Literal pattern: case 42, case "hello"
    Literal(Type),
    /// Class pattern: case int(), case str()
    Class(Type),
    /// Capture pattern: case x
    Capture(String),
    /// Wildcard pattern: case _
    Wildcard,
    /// Or pattern: case 1 | 2 | 3
    Or(Vec<MatchPattern>),
    /// Sequence pattern: case [x, y, z]
    Sequence(Vec<MatchPattern>),
    /// Mapping pattern: case {"key": value}
    Mapping(Vec<(String, MatchPattern)>),
    /// As pattern: case int() as n
    As(Box<MatchPattern>, String),
    /// Guard: case x if x > 0
    Guard(Box<MatchPattern>, String), // guard expr as string
}

/// Parse a match case pattern and derive narrowing condition
#[allow(dead_code)]
pub fn parse_match_pattern(
    source: &str,
    subject_var: &str,
    pattern_node: &tree_sitter::Node,
) -> NarrowingCondition {
    let node_text =
        |n: &tree_sitter::Node| -> &str { n.utf8_text(source.as_bytes()).unwrap_or("") };

    match pattern_node.kind() {
        // Class pattern: case int(), case str(), case MyClass()
        "class_pattern" => {
            if let Some(class_node) = pattern_node.child_by_field_name("class") {
                let class_name = node_text(&class_node);
                let target_type = parse_simple_type_name(class_name);
                return NarrowingCondition::IsInstance {
                    var_name: subject_var.to_string(),
                    types: vec![target_type],
                };
            }
        }
        // Literal pattern: case 42, case "hello", case None
        "none" => {
            return NarrowingCondition::IsNone {
                var_name: subject_var.to_string(),
            };
        }
        "integer" | "float" | "string" | "true" | "false" => {
            let _pattern_text = node_text(pattern_node);
            let ty = match pattern_node.kind() {
                "integer" => Type::Int,
                "float" => Type::Float,
                "string" => Type::Str,
                "true" | "false" => Type::Bool,
                _ => Type::Unknown,
            };
            return NarrowingCondition::Equals {
                var_name: subject_var.to_string(),
                value: ty,
            };
        }
        // As pattern: case int() as n
        "as_pattern" => {
            // Get the inner pattern and recurse
            if let Some(pattern) = pattern_node.child_by_field_name("pattern") {
                return parse_match_pattern(source, subject_var, &pattern);
            }
        }
        // Or pattern: case int | str
        "or_pattern" | "union_pattern" => {
            let mut types = Vec::new();
            let mut cursor = pattern_node.walk();
            for child in pattern_node.children(&mut cursor) {
                if child.kind() != "|" {
                    // Try to get type from each alternative
                    if let NarrowingCondition::IsInstance { types: t, .. } =
                        parse_match_pattern(source, subject_var, &child)
                    {
                        types.extend(t);
                    }
                }
            }
            if !types.is_empty() {
                return NarrowingCondition::IsInstance {
                    var_name: subject_var.to_string(),
                    types,
                };
            }
        }
        // Wildcard or capture doesn't narrow
        "wildcard_pattern" | "capture_pattern" => {
            return NarrowingCondition::Unknown;
        }
        _ => {}
    }

    NarrowingCondition::Unknown
}

/// Parse a match statement and get the subject variable
#[allow(dead_code)]
pub fn get_match_subject(source: &str, node: &tree_sitter::Node) -> Option<String> {
    if node.kind() != "match_statement" {
        return None;
    }

    if let Some(subject) = node.child_by_field_name("subject") {
        if subject.kind() == "identifier" {
            let text = subject.utf8_text(source.as_bytes()).ok()?;
            return Some(text.to_string());
        }
    }

    None
}
