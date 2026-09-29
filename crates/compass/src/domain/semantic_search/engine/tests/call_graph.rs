use super::*;

#[test]
fn test_build_call_graph() {
    use crate::syntax::Language;

    let mut engine = SemanticSearchEngine::new();
    let file = PathBuf::from("test.py");

    let code = r#"
def func1():
    func2()
    func3()

def func2():
    func3()

def func3():
    pass
"#;

    // Build call graph
    let result = engine.build_call_graph(file.clone(), code, Language::Python);
    assert!(result.is_ok());

    // Verify call graph contains expected relationships
    let callers_of_func2 = engine.call_graph.get_callers("func2");
    assert_eq!(callers_of_func2.len(), 1);
    assert_eq!(callers_of_func2[0].caller, "func1");

    let callers_of_func3 = engine.call_graph.get_callers("func3");
    assert_eq!(callers_of_func3.len(), 2); // Called by func1 and func2
}

#[test]
fn test_call_hierarchy_search() {
    use crate::syntax::Language;

    let mut engine = SemanticSearchEngine::new();
    let file = PathBuf::from("test.py");

    let code = r#"
def level1():
    level2()

def level2():
    level3()

def level3():
    pass
"#;

    // Build call graph
    engine
        .build_call_graph(file.clone(), code, Language::Python)
        .unwrap();

    // Search for callers of level3
    let query = SearchQuery {
        kind: SearchKind::CallHierarchy {
            symbol: "level3".to_string(),
            file: file.clone(),
            direction: CallDirection::Callers,
        },
        scope: SearchScope::Project,
        max_results: 100,
    };

    let result = engine.search(&query);
    assert!(!result.is_empty());

    // Call hierarchy is recursive, so we find level2 (direct caller) and level1 (indirect)
    let caller_names: Vec<String> = result
        .matches
        .iter()
        .filter_map(|m| m.symbol.clone())
        .collect();
    assert!(caller_names.contains(&"level2".to_string())); // Direct caller
    assert!(caller_names.contains(&"level1".to_string())); // Indirect caller
}

#[test]
fn test_call_hierarchy_multi_level() {
    use crate::syntax::Language;

    let mut engine = SemanticSearchEngine::new();
    let file = PathBuf::from("test.py");

    let code = r#"
def top():
    middle1()
    middle2()

def middle1():
    bottom()

def middle2():
    bottom()

def bottom():
    pass
"#;

    // Build call graph
    engine
        .build_call_graph(file.clone(), code, Language::Python)
        .unwrap();

    // Search for callers of bottom (should find middle1 and middle2)
    let query = SearchQuery {
        kind: SearchKind::CallHierarchy {
            symbol: "bottom".to_string(),
            file: file.clone(),
            direction: CallDirection::Callers,
        },
        scope: SearchScope::Project,
        max_results: 100,
    };

    let result = engine.search(&query);
    assert!(!result.is_empty());

    // Call hierarchy is recursive, finds middle1, middle2 (direct callers) and top (indirect)
    let caller_names: Vec<String> = result
        .matches
        .iter()
        .filter_map(|m| m.symbol.clone())
        .collect();
    assert!(caller_names.contains(&"middle1".to_string())); // Direct caller
    assert!(caller_names.contains(&"middle2".to_string())); // Direct caller
    assert!(caller_names.contains(&"top".to_string())); // Indirect caller
}

#[test]
fn test_call_hierarchy_callees() {
    use crate::syntax::Language;

    let mut engine = SemanticSearchEngine::new();
    let file = PathBuf::from("test.py");

    let code = r#"
def caller():
    callee1()
    callee2()
    callee3()

def callee1():
    pass

def callee2():
    pass

def callee3():
    pass
"#;

    // Build call graph
    engine
        .build_call_graph(file.clone(), code, Language::Python)
        .unwrap();

    // Search for callees of caller
    let query = SearchQuery {
        kind: SearchKind::CallHierarchy {
            symbol: "caller".to_string(),
            file: file.clone(),
            direction: CallDirection::Callees,
        },
        scope: SearchScope::Project,
        max_results: 100,
    };

    let result = engine.search(&query);
    assert_eq!(result.len(), 3);

    let callee_names: Vec<String> = result
        .matches
        .iter()
        .filter_map(|m| m.symbol.clone())
        .collect();
    assert!(callee_names.contains(&"callee1".to_string()));
    assert!(callee_names.contains(&"callee2".to_string()));
    assert!(callee_names.contains(&"callee3".to_string()));
}
