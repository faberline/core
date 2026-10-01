use super::*;
use crate::domain::ts_type_system::types::TsInterface;

fn empty_ctx() -> TsTypeContext {
    TsTypeContext::new()
}

// -----------------------------------------------------------------------
// Template literal matching
// -----------------------------------------------------------------------

#[test]
fn test_template_match_string_interpolation() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);

    let template = TsTemplateLiteralType::new("event-", Type::Str, "");
    assert!(inf.matches_template_literal(&template, "event-click"));
    assert!(inf.matches_template_literal(&template, "event-mousedown"));
    assert!(!inf.matches_template_literal(&template, "noevent"));
}

#[test]
fn test_template_match_number_interpolation() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);

    let template = TsTemplateLiteralType::new("col-", Type::Int, "");
    assert!(inf.matches_template_literal(&template, "col-12"));
    assert!(!inf.matches_template_literal(&template, "col-abc"));
}

#[test]
fn test_template_no_interpolation() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);

    let template = TsTemplateLiteralType {
        parts: vec![TemplatePart::Literal("hello".to_string())],
    };
    assert!(inf.matches_template_literal(&template, "hello"));
    assert!(!inf.matches_template_literal(&template, "world"));
}

// -----------------------------------------------------------------------
// Conditional type evaluation
// -----------------------------------------------------------------------

#[test]
fn test_conditional_true_branch() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);

    let cond = TsConditionalType {
        check_type: Box::new(Type::Str),
        extends_type: Box::new(Type::Str),
        true_type: Box::new(Type::Bool),
        false_type: Box::new(Type::Int),
    };
    let result = inf.evaluate_conditional_type(&cond, &HashMap::new());
    assert_eq!(result, Type::Bool);
}

#[test]
fn test_conditional_false_branch() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);

    let cond = TsConditionalType {
        check_type: Box::new(Type::Int),
        extends_type: Box::new(Type::Str),
        true_type: Box::new(Type::Bool),
        false_type: Box::new(Type::Float),
    };
    let result = inf.evaluate_conditional_type(&cond, &HashMap::new());
    assert_eq!(result, Type::Float);
}

// -----------------------------------------------------------------------
// Mapped type evaluation
// -----------------------------------------------------------------------

#[test]
fn test_mapped_type_keyof_interface() {
    let mut ctx = TsTypeContext::new();
    let mut iface = TsInterface::new("User".to_string());
    iface.properties.insert("name".to_string(), Type::Str);
    iface.properties.insert("age".to_string(), Type::Int);
    ctx.register_interface(iface);

    let inf = AdvancedTsTypeInferencer::new(&ctx);

    let mapped = TsMappedType {
        key_var: "K".to_string(),
        keys: Type::Str,
        value_type: Type::Optional(Box::new(Type::Str)),
        optional_modifier: None,
        readonly_modifier: None,
    };

    let result = inf.evaluate_mapped_type(&mapped, "User");
    assert_eq!(result.len(), 2);
    assert!(result.contains_key("name"));
    assert!(result.contains_key("age"));
}

#[test]
fn test_keyof_unknown_interface() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);
    assert!(inf.keyof("NonExistent").is_empty());
}

// -----------------------------------------------------------------------
// Generic call inference
// -----------------------------------------------------------------------

#[test]
fn test_infer_identity_function() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);

    let type_params = vec![TsTypeParam::new("T".to_string())];
    // function identity<T>(x: T): T  →  identity("hello")
    let param_types = vec![Type::TypeVar {
        id: TypeVarId(0),
        name: "T".to_string(),
        bound: None,
        constraints: vec![],
        variance: crate::type_inference::Variance::Invariant,
    }];
    let arg_types = vec![Type::Str];

    let subs = inf
        .infer_generic_call(&type_params, &param_types, &arg_types)
        .expect("inference should succeed");
    assert_eq!(subs.get(&TypeVarId(0)), Some(&Type::Str));
}

#[test]
fn test_infer_arity_mismatch() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);
    let type_params = vec![TsTypeParam::new("T".to_string())];
    let result = inf.infer_generic_call(&type_params, &[Type::Str], &[]);
    assert!(result.is_err());
}

#[test]
fn test_infer_constraint_violation() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);

    // function add<T extends number>(x: T): T  →  add("text")  ← error
    let type_params = vec![TsTypeParam::new("T".to_string()).with_constraint(Type::Float)];
    let param_types = vec![Type::TypeVar {
        id: TypeVarId(0),
        name: "T".to_string(),
        bound: Some(Box::new(Type::Float)),
        constraints: vec![],
        variance: crate::type_inference::Variance::Invariant,
    }];
    let arg_types = vec![Type::Str];

    let result = inf.infer_generic_call(&type_params, &param_types, &arg_types);
    assert!(result.is_err(), "should fail constraint check");
}

// -----------------------------------------------------------------------
// check_extends
// -----------------------------------------------------------------------

#[test]
fn test_check_extends_satisfied() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);
    assert_eq!(
        inf.check_extends(&Type::Int, &Type::Float),
        ConstraintResult::Satisfied
    );
}

#[test]
fn test_check_extends_violated() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);
    let result = inf.check_extends(&Type::Str, &Type::Int);
    assert!(matches!(result, ConstraintResult::Violated { .. }));
}

#[test]
fn test_check_extends_unknown_for_typevar() {
    let ctx = empty_ctx();
    let inf = AdvancedTsTypeInferencer::new(&ctx);
    let tv = Type::TypeVar {
        id: TypeVarId(0),
        name: "T".to_string(),
        bound: None,
        constraints: vec![],
        variance: crate::type_inference::Variance::Invariant,
    };
    assert_eq!(
        inf.check_extends(&tv, &Type::Str),
        ConstraintResult::Unknown
    );
}
