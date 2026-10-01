use super::*;

fn make_inferencer() -> TsTypeInferencer<'static> {
    TsTypeInferencer::new("")
}

#[test]
fn test_fresh_type_var() {
    let mut inf = make_inferencer();
    let tv1 = inf.fresh_type_var("T");
    let tv2 = inf.fresh_type_var("U");

    match (&tv1, &tv2) {
        (Type::TypeVar { id: id1, .. }, Type::TypeVar { id: id2, .. }) => {
            assert_ne!(id1, id2);
        }
        _ => panic!("Expected TypeVar"),
    }
}

#[test]
fn test_type_context_interface() {
    let mut ctx = TsTypeContext::new();

    let mut iface = TsInterface::new("Readable".to_string());
    iface.methods.insert(
        "read".to_string(),
        Type::Callable {
            params: vec![],
            ret: Box::new(Type::Str),
        },
    );
    ctx.register_interface(iface);

    let resolved = ctx.resolve_type("Readable");
    assert!(matches!(resolved, Some(Type::Protocol { .. })));
}

#[test]
fn test_structural_compatibility() {
    let mut inf = make_inferencer();

    // Register interface
    let mut iface = TsInterface::new("Named".to_string());
    iface.properties.insert("name".to_string(), Type::Str);
    inf.context_mut().register_interface(iface);

    // Check object literal
    let obj = Type::Protocol {
        name: "".to_string(),
        module: None,
        members: vec![("name".to_string(), Type::Str)],
    };

    assert!(inf.check_structural_compatibility(&obj, "Named"));

    // Missing property
    let obj2 = Type::Protocol {
        name: "".to_string(),
        module: None,
        members: vec![],
    };
    assert!(!inf.check_structural_compatibility(&obj2, "Named"));
}

#[test]
fn test_union_type_handling() {
    let union = Type::Union(vec![Type::Str, Type::Int]);

    // Should be assignable to wider union
    let _wider = Type::Union(vec![Type::Str, Type::Int, Type::Bool]);
    assert!(is_assignable_to(&Type::Str, &union));
    assert!(!is_assignable_to(&Type::Bool, &union));
}

#[test]
fn test_partial_type() {
    let inf = make_inferencer();

    let proto = Type::Protocol {
        name: "Person".to_string(),
        module: None,
        members: vec![
            ("name".to_string(), Type::Str),
            ("age".to_string(), Type::Int),
        ],
    };

    let partial = inf.make_partial(proto);

    match partial {
        Type::Protocol { members, .. } => {
            assert!(members
                .iter()
                .all(|(_, ty)| matches!(ty, Type::Optional(_))));
        }
        _ => panic!("Expected Protocol"),
    }
}

#[test]
fn test_bind_and_lookup() {
    let mut inf = make_inferencer();
    inf.bind_variable("x".to_string(), Type::Int);

    let result = inf.context.variables.get("x");
    assert_eq!(result, Some(&Type::Int));
}
