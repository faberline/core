use crate::domain::narrowing::condition::NarrowingCondition;
use crate::domain::type_system::ty::Type;

/// Parse an assert statement and extract the narrowing condition
/// e.g., `assert isinstance(x, int)` -> IsInstance { var_name: "x", types: [int] }
/// e.g., `assert x is not None` -> IsNotNone { var_name: "x" }
#[allow(dead_code)]
pub fn parse_assert(source: &str, node: &tree_sitter::Node) -> NarrowingCondition {
    // assert statements have the condition as the first child
    if node.kind() != "assert_statement" {
        return NarrowingCondition::Unknown;
    }

    // Get the condition (skip "assert" keyword)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "assert" {
            return parse_condition(source, &child);
        }
    }

    NarrowingCondition::Unknown
}

/// Parse a condition expression into a NarrowingCondition
pub fn parse_condition(source: &str, node: &tree_sitter::Node) -> NarrowingCondition {
    let node_text =
        |n: &tree_sitter::Node| -> &str { n.utf8_text(source.as_bytes()).unwrap_or("") };

    match node.kind() {
        "comparison_operator" => parse_comparison(source, node),
        "boolean_operator" => parse_boolean_op(source, node),
        "not_operator" => {
            if let Some(arg) = node.child(1) {
                NarrowingCondition::Not(Box::new(parse_condition(source, &arg)))
            } else {
                NarrowingCondition::Unknown
            }
        }
        "call" => parse_call_condition(source, node),
        "identifier" => {
            // Bare identifier is a truthiness check
            NarrowingCondition::Truthy {
                var_name: node_text(node).to_string(),
            }
        }
        _ => NarrowingCondition::Unknown,
    }
}

#[allow(dead_code)]
fn parse_comparison(source: &str, node: &tree_sitter::Node) -> NarrowingCondition {
    let node_text =
        |n: &tree_sitter::Node| -> &str { n.utf8_text(source.as_bytes()).unwrap_or("") };

    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    if children.len() >= 3 {
        let left = &children[0];
        let op = node_text(&children[1]);
        let right = &children[2];

        let left_text = node_text(left);
        let right_text = node_text(right);

        match op {
            "is" => {
                if right_text == "None" && left.kind() == "identifier" {
                    return NarrowingCondition::IsNone {
                        var_name: left_text.to_string(),
                    };
                }
                // Handle type(x) is T
                if let Some(type_check) = parse_type_check(source, left, right, false) {
                    return type_check;
                }
            }
            "is not" => {
                if right_text == "None" && left.kind() == "identifier" {
                    return NarrowingCondition::IsNotNone {
                        var_name: left_text.to_string(),
                    };
                }
                // Handle type(x) is not T
                if let Some(type_check) = parse_type_check(source, left, right, true) {
                    return type_check;
                }
            }
            "==" => {
                if left.kind() == "identifier" && right_text == "None" {
                    return NarrowingCondition::IsNone {
                        var_name: left_text.to_string(),
                    };
                }
            }
            "!=" => {
                if left.kind() == "identifier" && right_text == "None" {
                    return NarrowingCondition::IsNotNone {
                        var_name: left_text.to_string(),
                    };
                }
            }
            _ => {}
        }
    }

    NarrowingCondition::Unknown
}

/// Parse type(x) is T or type(x) is not T patterns
#[allow(dead_code)]
fn parse_type_check(
    source: &str,
    left: &tree_sitter::Node,
    right: &tree_sitter::Node,
    negated: bool,
) -> Option<NarrowingCondition> {
    let node_text =
        |n: &tree_sitter::Node| -> &str { n.utf8_text(source.as_bytes()).unwrap_or("") };

    // Check if left is type(x)
    if left.kind() == "call" {
        if let Some(func) = left.child_by_field_name("function") {
            if node_text(&func) == "type" {
                if let Some(args) = left.child_by_field_name("arguments") {
                    let mut cursor = args.walk();
                    let arg_nodes: Vec<_> = args
                        .children(&mut cursor)
                        .filter(|n| n.kind() != "(" && n.kind() != ")" && n.kind() != ",")
                        .collect();

                    if !arg_nodes.is_empty() && arg_nodes[0].kind() == "identifier" {
                        let var_name = node_text(&arg_nodes[0]).to_string();
                        let target_type = parse_simple_type_name(node_text(right));

                        if negated {
                            return Some(NarrowingCondition::NotTypeCheck {
                                var_name,
                                target_type,
                            });
                        } else {
                            return Some(NarrowingCondition::TypeCheck {
                                var_name,
                                target_type,
                            });
                        }
                    }
                }
            }
        }
    }

    None
}

#[allow(dead_code)]
fn parse_boolean_op(source: &str, node: &tree_sitter::Node) -> NarrowingCondition {
    let node_text =
        |n: &tree_sitter::Node| -> &str { n.utf8_text(source.as_bytes()).unwrap_or("") };

    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    if children.len() >= 3 {
        let left = &children[0];
        let op = node_text(&children[1]);
        let right = &children[2];

        let left_cond = parse_condition(source, left);
        let right_cond = parse_condition(source, right);

        match op {
            "and" => NarrowingCondition::And(Box::new(left_cond), Box::new(right_cond)),
            "or" => NarrowingCondition::Or(Box::new(left_cond), Box::new(right_cond)),
            _ => NarrowingCondition::Unknown,
        }
    } else {
        NarrowingCondition::Unknown
    }
}

#[allow(dead_code)]
fn parse_call_condition(source: &str, node: &tree_sitter::Node) -> NarrowingCondition {
    let node_text =
        |n: &tree_sitter::Node| -> &str { n.utf8_text(source.as_bytes()).unwrap_or("") };

    let func = match node.child_by_field_name("function") {
        Some(f) => f,
        None => return NarrowingCondition::Unknown,
    };

    let func_name = node_text(&func);

    let args = match node.child_by_field_name("arguments") {
        Some(a) => a,
        None => return NarrowingCondition::Unknown,
    };

    let mut cursor = args.walk();
    let arg_nodes: Vec<_> = args
        .children(&mut cursor)
        .filter(|n| n.kind() != "(" && n.kind() != ")" && n.kind() != ",")
        .collect();

    match func_name {
        "isinstance" => {
            if arg_nodes.len() >= 2 {
                let var_node = &arg_nodes[0];
                let type_node = &arg_nodes[1];

                if var_node.kind() == "identifier" {
                    let var_name = node_text(var_node).to_string();
                    let types = parse_isinstance_types(source, type_node);
                    return NarrowingCondition::IsInstance { var_name, types };
                }
            }
        }
        "hasattr" => {
            // hasattr(x, "attr_name")
            if arg_nodes.len() >= 2 {
                let var_node = &arg_nodes[0];
                let attr_node = &arg_nodes[1];

                if var_node.kind() == "identifier" && attr_node.kind() == "string" {
                    let var_name = node_text(var_node).to_string();
                    // Extract string content without quotes
                    let attr_text = node_text(attr_node);
                    let attr_name = attr_text
                        .trim_start_matches(|c| c == '"' || c == '\'')
                        .trim_end_matches(|c| c == '"' || c == '\'')
                        .to_string();
                    return NarrowingCondition::HasAttr {
                        var_name,
                        attr_name,
                    };
                }
            }
        }
        "callable" => {
            // callable(x)
            if !arg_nodes.is_empty() {
                let var_node = &arg_nodes[0];
                if var_node.kind() == "identifier" {
                    let var_name = node_text(var_node).to_string();
                    return NarrowingCondition::IsCallable { var_name };
                }
            }
        }
        _ => {}
    }

    NarrowingCondition::Unknown
}

#[allow(dead_code)]
fn parse_isinstance_types(source: &str, node: &tree_sitter::Node) -> Vec<Type> {
    let node_text =
        |n: &tree_sitter::Node| -> &str { n.utf8_text(source.as_bytes()).unwrap_or("") };

    match node.kind() {
        "identifier" => {
            let name = node_text(node);
            vec![parse_simple_type_name(name)]
        }
        "tuple" => {
            let mut types = Vec::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    types.push(parse_simple_type_name(node_text(&child)));
                }
            }
            types
        }
        _ => vec![Type::Unknown],
    }
}

#[allow(dead_code)]
pub(super) fn parse_simple_type_name(name: &str) -> Type {
    match name {
        "int" => Type::Int,
        "float" => Type::Float,
        "str" => Type::Str,
        "bool" => Type::Bool,
        "bytes" => Type::Bytes,
        "list" => Type::list(Type::Unknown),
        "dict" => Type::dict(Type::Unknown, Type::Unknown),
        "set" => Type::Set(Box::new(Type::Unknown)),
        "tuple" => Type::Tuple(vec![]),
        _ => Type::Instance {
            name: name.to_string(),
            module: None,
            type_args: vec![],
        },
    }
}
