use super::*;

#[test]
fn test_extract_function_no_params() {
    let source = r#"print("Hello")
print("World")"#;
    let request = RefactorRequest {
        kind: RefactorKind::ExtractFunction {
            name: "greet".to_string(),
        },
        file: PathBuf::from("test.py"),
        span: Span::new(0, 14), // print("Hello")
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(!result.has_errors());
    assert!(result.has_changes());

    // Should have 2 edits: insert function definition + replace with call
    let edits = result.file_edits.get(&PathBuf::from("test.py"));
    assert!(edits.is_some());
    assert_eq!(edits.unwrap().len(), 2);
}

#[test]
fn test_extract_function_with_params() {
    let source = r#"x = 5
y = 10
result = x + y
print(result)"#;
    let request = RefactorRequest {
        kind: RefactorKind::ExtractFunction {
            name: "add_numbers".to_string(),
        },
        file: PathBuf::from("test.py"),
        span: Span::new(12, 26), // "result = x + y"
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    // Print diagnostics for debugging
    for diag in &result.diagnostics {
        eprintln!("[{:?}] {}", diag.level, diag.message);
    }

    if result.has_errors() {
        panic!("Unexpected errors in result");
    }

    assert!(result.has_changes());

    // Check diagnostic message contains parameter count
    let info_diag = result
        .diagnostics
        .iter()
        .find(|d| d.level == DiagnosticLevel::Info);
    assert!(info_diag.is_some());
    let msg = &info_diag.unwrap().message;
    assert!(msg.contains("parameter"));
}

#[test]
fn test_extract_function_with_return() {
    let source = r#"def process():
    data = "test"
    result = data.upper()
    return result"#;
    let request = RefactorRequest {
        kind: RefactorKind::ExtractFunction {
            name: "transform".to_string(),
        },
        file: PathBuf::from("test.py"),
        span: Span::new(23, 48), // "data = "test"\n    result = data.upper()"
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(!result.has_errors());
    assert!(result.has_changes());
}

#[test]
fn test_extract_function_invalid_name() {
    let source = r#"print("test")"#;
    let request = RefactorRequest {
        kind: RefactorKind::ExtractFunction {
            name: "def".to_string(), // Python keyword
        },
        file: PathBuf::from("test.py"),
        span: Span::new(0, 13),
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(result.has_errors());
    assert!(!result.has_changes());
}

#[test]
fn test_extract_function_empty_name() {
    let source = r#"print("test")"#;
    let request = RefactorRequest {
        kind: RefactorKind::ExtractFunction {
            name: "".to_string(), // Empty name
        },
        file: PathBuf::from("test.py"),
        span: Span::new(0, 13),
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(result.has_errors());
    assert!(!result.has_changes());
}

#[test]
fn test_extract_function_with_type_annotations() {
    let source = r#"x = 5
y = 10
result = x + y
print(result)"#;
    let request = RefactorRequest {
        kind: RefactorKind::ExtractFunction {
            name: "calculate".to_string(),
        },
        file: PathBuf::from("test.py"),
        span: Span::new(12, 26), // "result = x + y"
        options: RefactorOptions {
            add_type_annotations: true,
            ..Default::default()
        },
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(!result.has_errors());
    assert!(result.has_changes());

    // Check that type annotations were added
    let edits = result.file_edits.get(&PathBuf::from("test.py"));
    assert!(edits.is_some());

    // The function definition should contain type annotations
    let func_def_edit = &edits.unwrap()[0];
    assert!(func_def_edit.new_text.contains(": Any"));
    assert!(
        func_def_edit.new_text.contains("-> Any") || func_def_edit.new_text.contains("-> tuple")
    );
}
