//! Tests for type inference

use std::collections::HashMap;

use super::*;
use crate::syntax::MultiParser;

mod classes;
mod expressions;
mod imports;
mod special_classes;

fn infer_type(code: &str) -> Type {
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    // Find first expression
    let root = parsed.tree.root_node();
    if let Some(stmt) = root.child(0) {
        if stmt.kind() == "expression_statement" {
            if let Some(expr) = stmt.child(0) {
                return inferencer.infer_expr(&expr);
            }
        }
    }
    Type::Unknown
}
