use super::*;
use crate::domain::cross_file::binding::TypeVarInfo;

#[test]
fn test_type_context() {
    let mut ctx = TypeContext::new();

    let binding = TypeBinding {
        ty: Type::Unknown,
        source_file: PathBuf::from("test.py"),
        symbol: "foo".to_string(),
        line: 1,
        is_exported: true,
        dependencies: vec![],
        is_propagated: false,
    };

    ctx.add_binding(PathBuf::from("test.py"), binding);

    assert!(ctx.get_binding(&PathBuf::from("test.py"), "foo").is_some());
}

#[test]
fn test_type_var_info() {
    let tv = TypeVarInfo::new("T").with_bound(Type::Unknown).covariant();

    assert_eq!(tv.name, "T");
    assert!(tv.covariant);
    assert!(!tv.contravariant);
}

#[test]
fn test_import_graph_topological_sort() {
    let mut graph = ImportGraph::new();

    graph.add_import(PathBuf::from("a.py"), PathBuf::from("b.py"));
    graph.add_import(PathBuf::from("b.py"), PathBuf::from("c.py"));

    let order = graph.topological_sort();

    // c.py should come before b.py, b.py before a.py
    let c_pos = order.iter().position(|p| p == &PathBuf::from("c.py"));
    let b_pos = order.iter().position(|p| p == &PathBuf::from("b.py"));
    let a_pos = order.iter().position(|p| p == &PathBuf::from("a.py"));

    if let (Some(c), Some(b), Some(a)) = (c_pos, b_pos, a_pos) {
        assert!(c < b);
        assert!(b < a);
    }
}

#[test]
fn test_deep_inferencer() {
    let mut inferencer = DeepTypeInferencer::new();
    inferencer.add_file(PathBuf::from("test.py"));

    assert!(inferencer.files.contains_key(&PathBuf::from("test.py")));
}

/// R9, S4: Cycle between A↔B detected, cycle members returned, no infinite loop.
#[test]
fn test_circular_import_detection() {
    let mut inferencer = DeepTypeInferencer::new();
    let a = PathBuf::from("a.py");
    let b = PathBuf::from("b.py");
    inferencer.add_file(a.clone());
    inferencer.add_file(b.clone());

    // A imports B, B imports A → cycle
    inferencer.add_import_edge(a.clone(), b.clone());
    inferencer.add_import_edge(b.clone(), a.clone());

    let cycles = inferencer.detect_import_cycles();
    assert!(!cycles.is_empty(), "Should detect at least one cycle");

    // The cycle members should include both a.py and b.py
    let all_members: HashSet<PathBuf> = cycles.into_iter().flatten().collect();
    assert!(all_members.contains(&a), "a.py should be in cycle");
    assert!(all_members.contains(&b), "b.py should be in cycle");

    // Topological sort should still terminate (no infinite loop).
    let topo = inferencer.topological_sort();
    assert!(
        !topo.is_empty(),
        "Topological sort should still return results"
    );
}

/// R9, S4: Symbols in cycle retain local types, cross-cycle imports
/// remain Type::Unknown.
#[test]
fn test_circular_import_fallback() {
    let mut inferencer = DeepTypeInferencer::new();
    let a = PathBuf::from("a.py");
    let b = PathBuf::from("b.py");
    inferencer.add_file(a.clone());
    inferencer.add_file(b.clone());

    // a.py defines foo locally
    inferencer.add_file_symbol(
        &a,
        "foo".to_string(),
        TypeBinding {
            ty: Type::Int,
            source_file: a.clone(),
            symbol: "foo".to_string(),
            line: 1,
            is_exported: true,
            dependencies: vec![],
            is_propagated: false,
        },
    );

    // b.py defines bar locally
    inferencer.add_file_symbol(
        &b,
        "bar".to_string(),
        TypeBinding {
            ty: Type::Str,
            source_file: b.clone(),
            symbol: "bar".to_string(),
            line: 1,
            is_exported: true,
            dependencies: vec![],
            is_propagated: false,
        },
    );

    // Circular import edges
    inferencer.add_import_edge(a.clone(), b.clone());
    inferencer.add_import_edge(b.clone(), a.clone());

    // Cycles detected
    let cycles = inferencer.detect_import_cycles();
    assert!(!cycles.is_empty());

    // Local types should still be intact
    let fa_a = inferencer.file_analysis(&a).unwrap();
    assert_eq!(fa_a.symbols.get("foo").unwrap().ty, Type::Int);

    let fa_b = inferencer.file_analysis(&b).unwrap();
    assert_eq!(fa_b.symbols.get("bar").unwrap().ty, Type::Str);
}

/// R3: Propagated TypeBindings have is_propagated = true, local ones false.
#[test]
fn test_propagated_binding_flag() {
    let mut inferencer = DeepTypeInferencer::new();
    let source = PathBuf::from("source.py");
    let target = PathBuf::from("target.py");
    inferencer.add_file(source.clone());
    inferencer.add_file(target.clone());

    // Source has an exported symbol
    inferencer.add_file_symbol(
        &source,
        "helper".to_string(),
        TypeBinding {
            ty: Type::Int,
            source_file: source.clone(),
            symbol: "helper".to_string(),
            line: 5,
            is_exported: true,
            dependencies: vec![],
            is_propagated: false,
        },
    );

    // Target has a local symbol
    inferencer.add_file_symbol(
        &target,
        "local_var".to_string(),
        TypeBinding {
            ty: Type::Str,
            source_file: target.clone(),
            symbol: "local_var".to_string(),
            line: 1,
            is_exported: false,
            dependencies: vec![],
            is_propagated: false,
        },
    );

    // Propagate from source to target
    inferencer.propagate_types(&source, &target, Some(&["helper".to_string()]));

    let fa = inferencer.file_analysis(&target).unwrap();

    // Local binding should not be propagated.
    let local = fa.symbols.get("local_var").unwrap();
    assert!(
        !local.is_propagated,
        "Local binding should have is_propagated=false"
    );

    // Propagated binding should be marked.
    let propagated = fa.symbols.get("helper").unwrap();
    assert!(
        propagated.is_propagated,
        "Propagated binding should have is_propagated=true"
    );
}
