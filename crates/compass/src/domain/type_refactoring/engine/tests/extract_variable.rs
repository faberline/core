use super::*;

#[test]
fn test_extract_variable() {
    let source = "result = user.name.upper()";
    let request = RefactorRequest {
        kind: RefactorKind::ExtractVariable {
            name: "temp_name".to_string(),
        },
        file: PathBuf::from("test.py"),
        span: Span::new(9, 26), // "user.name.upper()"
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(!result.has_errors());
    assert!(result.has_changes());

    // Should have 2 edits: insert assignment + replace expression
    let edits = result.file_edits.get(&PathBuf::from("test.py"));
    assert!(edits.is_some());
    assert_eq!(edits.unwrap().len(), 2);
}

#[test]
fn test_extract_variable_invalid_name() {
    let source = "result = 1 + 2";
    let request = RefactorRequest {
        kind: RefactorKind::ExtractVariable {
            name: "for".to_string(), // Python keyword
        },
        file: PathBuf::from("test.py"),
        span: Span::new(9, 14),
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(result.has_errors());
    assert!(!result.has_changes());
}
