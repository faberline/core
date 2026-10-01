use super::*;

#[test]
fn test_typing_import_integration() {
    let code = "from typing import List, Optional";
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    let root = parsed.tree.root_node();
    if let Some(import_node) = root.child(0) {
        inferencer.analyze_import(&import_node);
    }

    // Verify List and Optional are now in env
    assert!(inferencer.env().lookup("List").is_some());
    assert!(inferencer.env().lookup("Optional").is_some());
}

#[test]
fn test_collections_import_integration() {
    let code = "from collections import deque, Counter";
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    let root = parsed.tree.root_node();
    if let Some(import_node) = root.child(0) {
        inferencer.analyze_import(&import_node);
    }

    assert!(inferencer.env().lookup("deque").is_some());
    assert!(inferencer.env().lookup("Counter").is_some());
}

#[test]
fn test_import_with_alias() {
    let code = "from typing import List as L, Dict as D";
    let mut parser = MultiParser::new().unwrap();
    let parsed = parser.parse(code, crate::syntax::Language::Python).unwrap();
    let mut inferencer = TypeInferencer::new(code);

    let root = parsed.tree.root_node();
    if let Some(import_node) = root.child(0) {
        inferencer.analyze_import(&import_node);
    }

    // Should be available under aliases
    assert!(inferencer.env().lookup("L").is_some());
    assert!(inferencer.env().lookup("D").is_some());
    // Original names should not be bound
    assert!(inferencer.env().lookup("List").is_none());
    assert!(inferencer.env().lookup("Dict").is_none());
}
