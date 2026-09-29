use super::*;

fn make_extractor() -> TestExtractor {
    TestExtractor::new(TestExtractorConfig {
        type_mapping: vec![("QueryBuilder".to_string(), "RustQueryBuilder".to_string())],
        python_module: "cclab.titan".to_string(),
    })
}

#[test]
fn test_extract_simple_test() {
    let extractor = make_extractor();
    let source = r#"
#[test]
fn test_simple() {
    let x = 1;
    assert_eq!(x, 1);
}
"#;

    let tests = extractor
        .extract_tests_from_source(source, "test.rs".to_string())
        .unwrap();
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].name, "test_simple");
}

#[test]
fn test_translate_let() {
    let extractor = make_extractor();
    assert_eq!(extractor.translate_line("let x = 1;"), "x = 1");
    assert_eq!(extractor.translate_line("let mut x = 1;"), "x = 1");
}

#[test]
fn test_translate_vec() {
    let extractor = make_extractor();
    let result = extractor.translate_vec_macro(r#"vec!["a", "b"]"#);
    assert_eq!(result, r#"["a", "b"]"#);
}

#[test]
fn test_translate_assert_eq() {
    let extractor = make_extractor();
    let result = extractor.translate_assert_eq("assert_eq!(x, 1)");
    assert_eq!(result, "assert x == 1");
}

#[test]
fn test_translate_assert_eq_non_ascii() {
    let extractor = make_extractor();
    let result = extractor.translate_assert_eq(r#"assert_eq!("é", "ü")"#);
    assert_eq!(result, r#"assert "é" == "ü""#);
}

#[test]
fn test_translate_assert() {
    let extractor = make_extractor();
    let result = extractor.translate_assert("assert!(x > 0)");
    assert_eq!(result, "assert x > 0");
}

#[test]
fn test_translate_unwrap() {
    let extractor = make_extractor();
    let result = extractor.translate_line("let x = foo().unwrap();");
    assert_eq!(result, "x = foo()");
}

#[test]
fn test_translate_to_string() {
    let extractor = make_extractor();
    let result = extractor.translate_line(r#"let x = "hello".to_string();"#);
    assert_eq!(result, r#"x = "hello""#);
}

#[test]
fn test_translate_operator() {
    let extractor = make_extractor();
    let result = extractor.translate_operators("Operator::Eq");
    assert_eq!(result, "\"=\"");
}

#[test]
fn test_translate_extracted_value() {
    let extractor = make_extractor();
    let result = extractor.translate_extracted_value("ExtractedValue::Int(42)");
    assert_eq!(result, "42");
}

#[test]
fn test_translate_full_test() {
    let extractor = make_extractor();
    let source = r#"
#[test]
fn test_simple_select() {
    let qb = QueryBuilder::new("users").unwrap();
    let (sql, params) = qb.build_select();
    assert_eq!(sql, "SELECT * FROM \"users\"");
    assert_eq!(params.len(), 0);
}
"#;

    let tests = extractor
        .extract_tests_from_source(source, "test.rs".to_string())
        .unwrap();
    let python = extractor.translate_to_python(&tests[0]);

    assert!(python.contains("def test_simple_select():"));
    assert!(python.contains("qb = RustQueryBuilder(\"users\")"));
    assert!(python.contains("sql, params = qb.build_select()"));
    assert!(python.contains("assert sql == \"SELECT * FROM \\\"users\\\"\""));
}

#[test]
fn test_generate_multiple_tests() {
    let extractor = make_extractor();
    let source = r#"
#[test]
fn test_select_with_columns() {
    let qb = QueryBuilder::new("users").unwrap()
        .select(vec!["id".to_string(), "name".to_string()]).unwrap();
    let (sql, params) = qb.build_select();
    assert_eq!(sql, "SELECT \"id\", \"name\" FROM \"users\"");
    assert_eq!(params.len(), 0);
}

#[test]
fn test_select_with_where() {
    let qb = QueryBuilder::new("users").unwrap()
        .where_clause("id", Operator::Eq, ExtractedValue::Int(42)).unwrap();
    let (sql, params) = qb.build_select();
    assert_eq!(sql, "SELECT * FROM \"users\" WHERE \"id\" = $1");
    assert_eq!(params.len(), 1);
}
"#;

    let tests = extractor
        .extract_tests_from_source(source, "query.rs".to_string())
        .unwrap();
    assert_eq!(tests.len(), 2);

    let python = extractor.generate_test_file(&tests);

    // Check header
    assert!(python.contains("Auto-generated Python tests"));
    assert!(python.contains("from cclab.titan import *"));

    // Check first test
    assert!(python.contains("def test_select_with_columns():"));
    assert!(python.contains(r#".select(["id", "name"])"#));

    // Check second test
    assert!(python.contains("def test_select_with_where():"));
    assert!(python.contains(r#".where_clause("id", "=", 42)"#));

    // Print for visual inspection
    println!("\n=== Generated Python Test File ===\n{}", python);
}
