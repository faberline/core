use super::*;

#[test]
fn test_infer_literals() {
    assert_eq!(infer_type("42"), Type::Int);
    assert_eq!(infer_type("3.14"), Type::Float);
    assert_eq!(infer_type("\"hello\""), Type::Str);
    assert_eq!(infer_type("True"), Type::Bool);
    assert_eq!(infer_type("None"), Type::None);
}

#[test]
fn test_infer_binary_ops() {
    assert_eq!(infer_type("1 + 2"), Type::Int);
    assert_eq!(infer_type("1.0 + 2"), Type::Float);
    assert_eq!(infer_type("\"a\" + \"b\""), Type::Str);
    assert_eq!(infer_type("10 / 3"), Type::Float);
    assert_eq!(infer_type("10 // 3"), Type::Int);
}

#[test]
fn test_infer_containers() {
    assert_eq!(infer_type("[1, 2, 3]"), Type::list(Type::Int));
    assert_eq!(infer_type("{\"a\": 1}"), Type::dict(Type::Str, Type::Int));
    assert_eq!(
        infer_type("(1, \"a\")"),
        Type::Tuple(vec![Type::Int, Type::Str])
    );
}

#[test]
fn test_generic_call_inference() {
    use crate::domain::type_system::ty::{Param, ParamKind};

    // Test that calling a generic function infers type arguments
    // We'll manually create a generic function and test the inference

    // Create a generic identity function: def identity(x: T) -> T
    let t = Type::type_var(0, "T");
    let identity_fn = Type::Callable {
        params: vec![Param {
            name: "x".to_string(),
            ty: t.clone(),
            has_default: false,
            kind: ParamKind::Positional,
        }],
        ret: Box::new(t),
    };

    // Simulate unifying with Int argument
    let mut subs = HashMap::new();
    let param_ty = &identity_fn;
    if let Type::Callable { params, ret } = param_ty {
        // Unify parameter T with Int
        params[0].ty.unify(&Type::Int, &mut subs);

        // Apply substitution to return type
        let inferred_ret = ret.substitute(&subs);
        assert_eq!(inferred_ret, Type::Int);
    }
}

#[test]
fn test_generic_list_inference() {
    use crate::domain::type_system::ty::TypeVarId;

    // Test inferring element type from list[T] -> list[str]
    let t = Type::type_var(0, "T");
    let list_t = Type::list(t);

    let mut subs = HashMap::new();
    list_t.unify(&Type::list(Type::Str), &mut subs);

    // T should be inferred as Str
    assert_eq!(subs.get(&TypeVarId(0)), Some(&Type::Str));
}
