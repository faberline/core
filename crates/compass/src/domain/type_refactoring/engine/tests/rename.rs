use super::*;

#[test]
fn test_rename_symbol() {
    let source = "old_name = 42\nprint(old_name)";
    let request = RefactorRequest {
        kind: RefactorKind::Rename {
            new_name: "new_name".to_string(),
        },
        file: PathBuf::from("test.py"),
        span: Span::new(0, 8),
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(!result.has_errors());
    assert!(result.has_changes());

    // Should rename at least one occurrence
    let edits = result.file_edits.get(&PathBuf::from("test.py"));
    assert!(edits.is_some());
    assert!(edits.unwrap().len() >= 1);
}

#[test]
fn test_rename_to_keyword() {
    let source = "old_name = 42";
    let request = RefactorRequest {
        kind: RefactorKind::Rename {
            new_name: "class".to_string(), // Python keyword
        },
        file: PathBuf::from("test.py"),
        span: Span::new(0, 8),
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(result.has_errors());
    assert!(!result.has_changes());
}

#[test]
fn test_rename_same_name() {
    let source = "my_var = 42";
    let request = RefactorRequest {
        kind: RefactorKind::Rename {
            new_name: "my_var".to_string(), // Same name
        },
        file: PathBuf::from("test.py"),
        span: Span::new(0, 6),
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    // Should not error, but should not make changes
    assert!(!result.has_errors());
    assert!(!result.has_changes());
}
