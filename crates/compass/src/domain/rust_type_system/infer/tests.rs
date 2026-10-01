use super::*;

#[test]
fn test_fresh_type_var() {
    let mut ctx = RustTypeContext::new();
    let t1 = ctx.fresh_type_var("T");
    let t2 = ctx.fresh_type_var("U");

    match (&t1, &t2) {
        (
            RustType::TypeParam {
                id: id1,
                name: name1,
                ..
            },
            RustType::TypeParam {
                id: id2,
                name: name2,
                ..
            },
        ) => {
            assert_ne!(id1, id2);
            assert_eq!(name1, "T");
            assert_eq!(name2, "U");
        }
        _ => panic!("Expected TypeParam"),
    }
}

#[test]
fn test_type_binding() {
    let mut ctx = RustTypeContext::new();
    ctx.bind_type("x".to_string(), RustType::I32);

    assert_eq!(ctx.lookup_type("x"), Some(RustType::I32));
    assert_eq!(ctx.lookup_type("y"), None);
}

#[test]
fn test_unify_same_types() {
    let mut inferencer = RustTypeInferencer::new();

    let result = inferencer.unify(&RustType::I32, &RustType::I32);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), RustType::I32);
}

#[test]
fn test_unify_with_infer() {
    let mut inferencer = RustTypeInferencer::new();

    let result = inferencer.unify(&RustType::Infer, &RustType::I32);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), RustType::I32);
}

#[test]
fn test_unify_type_mismatch() {
    let mut inferencer = RustTypeInferencer::new();

    let result = inferencer.unify(&RustType::I32, &RustType::Str);
    assert!(result.is_err());
}
