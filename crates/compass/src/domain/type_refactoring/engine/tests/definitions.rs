use super::*;

#[test]
fn test_inline_symbol_simple() {
    let source = r#"temp = 42
result = temp * 2
print(result)"#;
    let request = RefactorRequest {
        kind: RefactorKind::Inline,
        file: PathBuf::from("test.py"),
        span: Span::new(0, 4), // "temp"
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(!result.has_errors());
    assert!(result.has_changes());

    // Should have edits for inlining
    let edits = result.file_edits.get(&PathBuf::from("test.py"));
    assert!(edits.is_some());
    assert!(edits.unwrap().len() >= 2); // Replace usage + remove definition
}

#[test]
fn test_change_signature_add_param() {
    let source = r#"def greet():
    print("Hello")"#;
    let request = RefactorRequest {
        kind: RefactorKind::ChangeSignature {
            changes: SignatureChanges {
                new_params: vec![("name".to_string(), Some("str".to_string()), None)],
                ..Default::default()
            },
        },
        file: PathBuf::from("test.py"),
        span: Span::new(0, 12), // "def greet():"
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(!result.has_errors());
    assert!(result.has_changes());

    // Should have edit for changing signature
    let edits = result.file_edits.get(&PathBuf::from("test.py"));
    assert!(edits.is_some());
    assert_eq!(edits.unwrap().len(), 1);
}

#[test]
fn test_move_definition_basic() {
    let source = r#"def helper():
    return 42

def main():
    result = helper()
    print(result)"#;
    let request = RefactorRequest {
        kind: RefactorKind::MoveDefinition {
            target_file: PathBuf::from("helpers.py"),
        },
        file: PathBuf::from("main.py"),
        span: Span::new(0, 26), // helper function
        options: RefactorOptions::default(),
    };

    let mut engine = RefactoringEngine::new();
    let result = engine.execute(&request, source);

    assert!(!result.has_errors());
    assert!(result.has_changes());

    // Should have edits for both files
    assert!(result.file_edits.contains_key(&PathBuf::from("main.py")));
    assert!(result.new_files.contains_key(&PathBuf::from("helpers.py")));
}
