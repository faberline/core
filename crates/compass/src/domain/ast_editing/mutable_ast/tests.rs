use super::*;

#[test]
fn test_mutable_node() {
    let mut root = MutableNode::new(NodeId(0), "module", Span::new(0, 100));
    let child = MutableNode::leaf(NodeId(1), "identifier", Span::new(0, 5), "foo");

    root.add_child(child);

    assert_eq!(root.children.len(), 1);
    assert_eq!(root.child(0).unwrap().kind, "identifier");
}

#[test]
fn test_copy_on_write() {
    let root = MutableNode::new(NodeId(0), "module", Span::new(0, 100));
    let mut root2 = root.clone();

    // Before modification, they share the same Arc
    assert!(Arc::ptr_eq(&root.children, &root2.children));

    // After modification, they don't
    root2.add_child(MutableNode::leaf(
        NodeId(1),
        "identifier",
        Span::new(0, 5),
        "foo",
    ));
    assert!(!Arc::ptr_eq(&root.children, &root2.children));
}

#[test]
fn test_undo_redo() {
    let root = MutableNode::new(NodeId(0), "module", Span::new(0, 100));
    let mut ast = MutableAst::new(root);

    // Take snapshot
    ast.snapshot();

    // Make change
    ast.root_mut().add_child(MutableNode::leaf(
        NodeId(1),
        "identifier",
        Span::new(0, 5),
        "foo",
    ));
    assert_eq!(ast.root().children.len(), 1);

    // Undo
    assert!(ast.undo());
    assert_eq!(ast.root().children.len(), 0);

    // Redo
    assert!(ast.redo());
    assert_eq!(ast.root().children.len(), 1);
}

#[test]
fn test_tree_diff() {
    let old = MutableNode::new(NodeId(0), "module", Span::new(0, 100));
    let mut new = MutableNode::new(NodeId(0), "module", Span::new(0, 100));
    new.add_child(MutableNode::leaf(
        NodeId(1),
        "identifier",
        Span::new(0, 5),
        "foo",
    ));

    let diff = TreeDiff::compute(&old, &new);
    assert_eq!(diff.len(), 1);
}
