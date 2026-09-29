use super::*;

#[test]
fn test_refactor_result() {
    let mut result = RefactorResult::empty();
    assert!(!result.has_changes());
    assert!(!result.has_errors());

    result.add_edit(
        PathBuf::from("test.py"),
        TextEdit {
            span: Span::new(0, 10),
            new_text: "new text".to_string(),
        },
    );
    assert!(result.has_changes());
}

#[test]
fn test_refactor_request() {
    let source = "old_name = 42";
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
}
