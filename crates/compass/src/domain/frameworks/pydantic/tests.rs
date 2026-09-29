use super::*;

#[test]
fn test_pydantic_provider() {
    let mut provider = PydanticTypeProvider::new();

    provider.register_model(PydanticModel {
        name: "UserModel".to_string(),
        fields: HashMap::new(),
        validators: Vec::new(),
        config: None,
    });

    assert!(provider.get_model("UserModel").is_some());
}

#[test]
fn test_pydantic_method_signatures() {
    let mut provider = PydanticTypeProvider::new();

    // Register a model
    provider.register_model(PydanticModel {
        name: "User".to_string(),
        fields: HashMap::new(),
        validators: Vec::new(),
        config: None,
    });

    let user_type = Type::Instance {
        name: "User".to_string(),
        module: None,
        type_args: Vec::new(),
    };

    // Test dict()
    let dict_sig = provider.get_method_signature(&user_type, "dict");
    assert!(dict_sig.is_some());
    let sig = dict_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Dict(..)));

    // Test json()
    let json_sig = provider.get_method_signature(&user_type, "json");
    assert!(json_sig.is_some());
    let sig = json_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Str));

    // Test copy()
    let copy_sig = provider.get_method_signature(&user_type, "copy");
    assert!(copy_sig.is_some());
    let sig = copy_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Instance { name, .. } if name == "User"));

    // Test parse_obj()
    let parse_obj_sig = provider.get_method_signature(&user_type, "parse_obj");
    assert!(parse_obj_sig.is_some());
    let sig = parse_obj_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Instance { name, .. } if name == "User"));
    assert_eq!(sig.params.len(), 1);

    // Test from_orm()
    let from_orm_sig = provider.get_method_signature(&user_type, "from_orm");
    assert!(from_orm_sig.is_some());
    let sig = from_orm_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Instance { name, .. } if name == "User"));
}
