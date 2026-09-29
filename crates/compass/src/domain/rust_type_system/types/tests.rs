use super::*;

#[test]
fn test_lifetime_outlives() {
    let static_lt = Lifetime::Static;
    let named_a = Lifetime::named(LifetimeId(0), "a");
    let named_b = Lifetime::named(LifetimeId(1), "b");

    assert!(static_lt.outlives(&named_a));
    assert!(static_lt.outlives(&named_b));
    assert!(!named_a.outlives(&static_lt));
    assert!(named_a.outlives(&named_a));
    assert!(!named_a.outlives(&named_b));
}

#[test]
fn test_rust_type_to_generic() {
    let int_type = RustType::I32;
    assert_eq!(int_type.to_generic_type(), Type::Int);

    let str_type = RustType::Str;
    assert_eq!(str_type.to_generic_type(), Type::Str);

    let ref_type = RustType::Reference {
        lifetime: None,
        mutable: false,
        inner: Box::new(RustType::I32),
    };
    assert_eq!(ref_type.to_generic_type(), Type::Int);
}

#[test]
fn test_struct_fields() {
    let struct_def = StructDef {
        name: "Point".to_string(),
        module: None,
        type_params: vec![],
        fields: StructFields::Named(vec![
            StructField {
                name: "x".to_string(),
                ty: RustType::F64,
                visibility: Visibility::Public,
            },
            StructField {
                name: "y".to_string(),
                ty: RustType::F64,
                visibility: Visibility::Public,
            },
        ]),
        where_bounds: vec![],
        visibility: Visibility::Public,
    };

    assert_eq!(struct_def.name, "Point");
    if let StructFields::Named(fields) = &struct_def.fields {
        assert_eq!(fields.len(), 2);
    } else {
        panic!("Expected named fields");
    }
}

#[test]
fn test_implements_trait_exact_match() {
    let clone_trait = TraitRef {
        trait_id: TraitId(1),
        name: "Clone".to_string(),
        type_args: vec![],
        lifetime_args: vec![],
    };

    let impl_block = ImplBlock {
        type_params: vec![],
        trait_ref: Some(clone_trait.clone()),
        self_type: RustType::I32,
        where_bounds: vec![],
        methods: vec![],
        associated_types: vec![],
        associated_consts: vec![],
        is_negative: false,
        is_unsafe: false,
    };

    let impls = vec![impl_block];

    // i32 should implement Clone
    assert!(RustType::I32.implements_trait(&clone_trait, &impls));

    // i64 should not implement Clone (no impl for it)
    assert!(!RustType::I64.implements_trait(&clone_trait, &impls));
}

#[test]
fn test_implements_trait_generic_impl() {
    use crate::type_inference::TypeVarId;

    let debug_trait = TraitRef {
        trait_id: TraitId(2),
        name: "Debug".to_string(),
        type_args: vec![],
        lifetime_args: vec![],
    };

    // impl<T> Debug for Vec<T>
    let generic_impl = ImplBlock {
        type_params: vec![RustTypeParam {
            name: "T".to_string(),
            id: TypeVarId(0),
            bounds: vec![],
            default: None,
        }],
        trait_ref: Some(debug_trait.clone()),
        self_type: RustType::Named {
            name: "Vec".to_string(),
            module: None,
            type_args: vec![RustType::TypeParam {
                id: TypeVarId(0),
                name: "T".to_string(),
                bounds: vec![],
            }],
            lifetime_args: vec![],
        },
        where_bounds: vec![],
        methods: vec![],
        associated_types: vec![],
        associated_consts: vec![],
        is_negative: false,
        is_unsafe: false,
    };

    let impls = vec![generic_impl];

    // Vec<i32> should implement Debug
    let vec_i32 = RustType::Named {
        name: "Vec".to_string(),
        module: None,
        type_args: vec![RustType::I32],
        lifetime_args: vec![],
    };
    assert!(vec_i32.implements_trait(&debug_trait, &impls));

    // Vec<String> should also implement Debug (generic impl)
    let vec_string = RustType::Named {
        name: "Vec".to_string(),
        module: None,
        type_args: vec![RustType::Named {
            name: "String".to_string(),
            module: None,
            type_args: vec![],
            lifetime_args: vec![],
        }],
        lifetime_args: vec![],
    };
    assert!(vec_string.implements_trait(&debug_trait, &impls));

    // HashMap should NOT implement Debug (wrong name)
    let hashmap = RustType::Named {
        name: "HashMap".to_string(),
        module: None,
        type_args: vec![RustType::I32, RustType::I32],
        lifetime_args: vec![],
    };
    assert!(!hashmap.implements_trait(&debug_trait, &impls));
}

#[test]
fn test_implements_trait_no_impls() {
    let send_trait = TraitRef {
        trait_id: TraitId(3),
        name: "Send".to_string(),
        type_args: vec![],
        lifetime_args: vec![],
    };

    // Empty impls - nothing implements Send
    let impls: Vec<ImplBlock> = vec![];
    assert!(!RustType::I32.implements_trait(&send_trait, &impls));
}

#[test]
fn test_implements_trait_wrong_trait() {
    let clone_trait = TraitRef {
        trait_id: TraitId(1),
        name: "Clone".to_string(),
        type_args: vec![],
        lifetime_args: vec![],
    };

    let copy_trait = TraitRef {
        trait_id: TraitId(2),
        name: "Copy".to_string(),
        type_args: vec![],
        lifetime_args: vec![],
    };

    // impl Clone for i32
    let impl_block = ImplBlock {
        type_params: vec![],
        trait_ref: Some(clone_trait.clone()),
        self_type: RustType::I32,
        where_bounds: vec![],
        methods: vec![],
        associated_types: vec![],
        associated_consts: vec![],
        is_negative: false,
        is_unsafe: false,
    };

    let impls = vec![impl_block];

    // i32 implements Clone but NOT Copy (different trait ID)
    assert!(RustType::I32.implements_trait(&clone_trait, &impls));
    assert!(!RustType::I32.implements_trait(&copy_trait, &impls));
}
