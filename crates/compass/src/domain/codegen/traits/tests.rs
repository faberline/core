use super::*;

#[test]
fn test_tech_stack_language() {
    assert_eq!(TechStack::Shield.language(), Language::Python);
    assert_eq!(TechStack::Titan.language(), Language::Python);
    assert_eq!(TechStack::Serde.language(), Language::Rust);
    assert_eq!(TechStack::Axum.language(), Language::Rust);
}

#[test]
fn test_tech_stack_from_str() {
    assert_eq!(TechStack::from_str("shield"), Some(TechStack::Shield));
    assert_eq!(TechStack::from_str("cclab.titan"), Some(TechStack::Titan));
    assert_eq!(TechStack::from_str("serde"), Some(TechStack::Serde));
    assert_eq!(TechStack::from_str("invalid"), None);
}

#[test]
fn test_gen_context_builder() {
    let ctx = GenContext::new(TechStack::Shield)
        .with_module("models")
        .with_docs(true)
        .with_type_mapping("CustomId", "str");

    assert_eq!(ctx.stack, TechStack::Shield);
    assert_eq!(ctx.module_name, Some("models".to_string()));
    assert!(ctx.generate_docs);
    assert_eq!(ctx.type_mappings.get("CustomId"), Some(&"str".to_string()));
}

#[test]
fn test_generated_code() {
    let code = GeneratedCode::new("user", "class User: pass", Language::Python)
        .with_imports(vec!["from cclab.shield import BaseModel".to_string()]);

    assert_eq!(code.filename(), "user.py");
    assert!(code.full_content().contains("from cclab.shield"));
}
