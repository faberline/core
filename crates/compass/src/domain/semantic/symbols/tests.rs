use super::*;

#[test]
fn test_type_info_display() {
    assert_eq!(TypeInfo::Primitive("int".to_string()).display(), "int");
    assert_eq!(
        TypeInfo::List(Box::new(TypeInfo::Primitive("str".to_string()))).display(),
        "list[str]"
    );
    assert_eq!(
        TypeInfo::Optional(Box::new(TypeInfo::Primitive("int".to_string()))).display(),
        "int?"
    );
}

#[test]
fn test_type_info_from_annotation() {
    assert_eq!(
        TypeInfo::from_python_annotation("int"),
        TypeInfo::Primitive("int".to_string())
    );
    assert_eq!(
        TypeInfo::from_python_annotation("List[str]"),
        TypeInfo::List(Box::new(TypeInfo::Primitive("str".to_string())))
    );
    assert_eq!(
        TypeInfo::from_python_annotation("Optional[int]"),
        TypeInfo::Optional(Box::new(TypeInfo::Primitive("int".to_string())))
    );
}

#[test]
fn test_rust_type_parsing() {
    assert_eq!(
        TypeInfo::from_rust_type("i32"),
        TypeInfo::Primitive("i32".to_string())
    );
    assert_eq!(
        TypeInfo::from_rust_type("&str"),
        TypeInfo::Reference(Box::new(TypeInfo::Primitive("str".to_string())))
    );
    assert_eq!(
        TypeInfo::from_rust_type("Option<String>"),
        TypeInfo::Optional(Box::new(TypeInfo::Named("String".to_string())))
    );
    assert_eq!(
        TypeInfo::from_rust_type("Vec<i32>"),
        TypeInfo::List(Box::new(TypeInfo::Primitive("i32".to_string())))
    );
    assert_eq!(
        TypeInfo::from_rust_type("Result<String, Error>"),
        TypeInfo::Generic(
            "Result".to_string(),
            vec![
                TypeInfo::Named("String".to_string()),
                TypeInfo::Named("Error".to_string()),
            ]
        )
    );
}

#[test]
fn test_generic_display_uses_angle_brackets() {
    let ty = TypeInfo::Generic(
        "Result".to_string(),
        vec![
            TypeInfo::Named("String".to_string()),
            TypeInfo::Named("Error".to_string()),
        ],
    );
    assert_eq!(ty.display(), "Result<String, Error>");
}
