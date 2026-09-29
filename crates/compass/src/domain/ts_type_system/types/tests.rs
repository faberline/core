use super::*;
use crate::type_inference::{TypeVarId, Variance};

#[test]
fn test_assignability_primitives() {
    assert!(is_assignable_to(&Type::Int, &Type::Int));
    assert!(is_assignable_to(&Type::Int, &Type::Float));
    assert!(!is_assignable_to(&Type::Float, &Type::Int));
    assert!(is_assignable_to(&Type::Str, &Type::Str));
}

#[test]
fn test_assignability_any() {
    assert!(is_assignable_to(&Type::Int, &Type::Any));
    assert!(is_assignable_to(&Type::Any, &Type::Str));
}

#[test]
fn test_assignability_never() {
    assert!(is_assignable_to(&Type::Never, &Type::Int));
    assert!(is_assignable_to(&Type::Never, &Type::Str));
}

#[test]
fn test_assignability_union() {
    let union = Type::Union(vec![Type::Int, Type::Str]);
    assert!(is_assignable_to(&Type::Int, &union));
    assert!(is_assignable_to(&Type::Str, &union));
    assert!(!is_assignable_to(&Type::Bool, &union));
}

#[test]
fn test_assignability_intersection() {
    let inter = Type::Intersection(vec![Type::Int, Type::Str]);
    // Nothing can satisfy both Int and Str, but Never can
    assert!(is_assignable_to(&Type::Never, &inter));
}

#[test]
fn test_literal_to_base() {
    use crate::type_inference::LiteralValue;
    let lit_str = Type::Literal(LiteralValue::Str("hello".to_string()));
    assert!(is_assignable_to(&lit_str, &Type::Str));

    let lit_int = Type::Literal(LiteralValue::Int(42));
    assert!(is_assignable_to(&lit_int, &Type::Int));
    assert!(is_assignable_to(&lit_int, &Type::Float));
}

#[test]
fn test_template_literal_evaluation() {
    use crate::type_inference::LiteralValue;

    let template = TsTemplateLiteralType::new(
        "Hello, ",
        Type::TypeVar {
            id: TypeVarId(0),
            name: "T".to_string(),
            bound: None,
            constraints: vec![],
            variance: Variance::Invariant,
        },
        "!",
    );

    let mut subs = HashMap::new();
    subs.insert(
        TypeVarId(0),
        Type::Literal(LiteralValue::Str("World".to_string())),
    );

    let result = template.evaluate(&subs);
    assert_eq!(
        result,
        Type::Literal(LiteralValue::Str("Hello, World!".to_string()))
    );
}

#[test]
fn test_interface_creation() {
    let mut iface = TsInterface::new("Readable".to_string());
    iface.methods.insert(
        "read".to_string(),
        Type::Callable {
            params: vec![],
            ret: Box::new(Type::Str),
        },
    );

    let props = iface.all_properties();
    assert!(props.is_empty()); // Methods don't show in properties
    assert_eq!(iface.methods.len(), 1);
}

#[test]
fn test_conditional_type() {
    let cond = TsConditionalType {
        check_type: Box::new(Type::Str),
        extends_type: Box::new(Type::Str),
        true_type: Box::new(Type::Bool),
        false_type: Box::new(Type::Int),
    };

    let result = cond.evaluate(&HashMap::new());
    assert_eq!(result, Type::Bool);
}
