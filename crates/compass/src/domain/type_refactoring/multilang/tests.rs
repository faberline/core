use super::*;

#[test]
fn test_language_detection() {
    assert_eq!(
        RefactorLanguage::from_path(&PathBuf::from("test.py")),
        Some(RefactorLanguage::Python)
    );
    assert_eq!(
        RefactorLanguage::from_path(&PathBuf::from("test.ts")),
        Some(RefactorLanguage::TypeScript)
    );
    assert_eq!(
        RefactorLanguage::from_path(&PathBuf::from("test.rs")),
        Some(RefactorLanguage::Rust)
    );
    assert_eq!(
        RefactorLanguage::from_path(&PathBuf::from("test.txt")),
        None
    );
}

#[test]
fn test_keyword_detection() {
    assert!(RefactorLanguage::Python.is_keyword("def"));
    assert!(RefactorLanguage::TypeScript.is_keyword("function"));
    assert!(RefactorLanguage::Rust.is_keyword("fn"));

    assert!(!RefactorLanguage::Python.is_keyword("foo"));
    assert!(!RefactorLanguage::TypeScript.is_keyword("bar"));
    assert!(!RefactorLanguage::Rust.is_keyword("baz"));
}

#[test]
fn test_extract_function_rust() {
    let refactorer = MultiLangRefactorer::new();
    let source = r#"
fn main() {
    let x = 1;
    let y = 2;
    let sum = x + y;
    println!("{}", sum);
}
"#;
    let request = RefactorRequest {
        kind: super::super::request::RefactorKind::ExtractFunction {
            name: "add_numbers".to_string(),
        },
        file: PathBuf::from("test.rs"),
        span: Span::new(35, 55),
        options: RefactorOptions::default(),
    };

    let result = refactorer.extract_function(&request, "add_numbers", source);
    assert!(!result.file_edits.is_empty());
}

#[test]
fn test_extract_function_typescript() {
    let refactorer = MultiLangRefactorer::new();
    let source = r#"
function main() {
    const x = 1;
    const y = 2;
    const sum = x + y;
    console.log(sum);
}
"#;
    let request = RefactorRequest {
        kind: super::super::request::RefactorKind::ExtractFunction {
            name: "addNumbers".to_string(),
        },
        file: PathBuf::from("test.ts"),
        span: Span::new(35, 60),
        options: RefactorOptions::default(),
    };

    let result = refactorer.extract_function(&request, "addNumbers", source);
    assert!(!result.file_edits.is_empty());
}

#[test]
fn test_keyword_validation_rust() {
    let refactorer = MultiLangRefactorer::new();
    let source = "fn main() { let x = 1; }";
    let request = RefactorRequest {
        kind: super::super::request::RefactorKind::ExtractFunction {
            name: "fn".to_string(), // Rust keyword
        },
        file: PathBuf::from("test.rs"),
        span: Span::new(12, 22),
        options: RefactorOptions::default(),
    };

    let result = refactorer.extract_function(&request, "fn", source);
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.level == DiagnosticLevel::Error));
}
