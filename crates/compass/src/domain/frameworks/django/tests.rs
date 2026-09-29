use super::*;
use crate::domain::frameworks::provider::FrameworkTypeProvider;

#[test]
fn test_django_provider() {
    let mut provider = DjangoTypeProvider::new();

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        DjangoField {
            name: "name".to_string(),
            field_type: DjangoFieldType::CharField,
            null: false,
            has_default: false,
        },
    );

    provider.register_model(DjangoModel {
        name: "User".to_string(),
        fields,
        relations: Vec::new(),
    });

    assert!(provider.get_model("User").is_some());
}

#[test]
fn test_django_queryset_methods() {
    let mut provider = DjangoTypeProvider::new();

    let mut fields = HashMap::new();
    fields.insert(
        "name".to_string(),
        DjangoField {
            name: "name".to_string(),
            field_type: DjangoFieldType::CharField,
            null: false,
            has_default: false,
        },
    );

    provider.register_model(DjangoModel {
        name: "User".to_string(),
        fields,
        relations: Vec::new(),
    });

    let queryset_type = Type::Instance {
        name: "UserQuerySet".to_string(),
        module: None,
        type_args: Vec::new(),
    };

    // Test filter() returns QuerySet
    let filter_sig = provider.get_method_signature(&queryset_type, "filter");
    assert!(filter_sig.is_some());
    let sig = filter_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Instance { name, .. } if name == "UserQuerySet"));

    // Test get() returns Model instance
    let get_sig = provider.get_method_signature(&queryset_type, "get");
    assert!(get_sig.is_some());
    let sig = get_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Instance { name, .. } if name == "User"));

    // Test count() returns int
    let count_sig = provider.get_method_signature(&queryset_type, "count");
    assert!(count_sig.is_some());
    let sig = count_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Int));

    // Test first() returns Optional[Model]
    let first_sig = provider.get_method_signature(&queryset_type, "first");
    assert!(first_sig.is_some());
    let sig = first_sig.unwrap();
    assert!(matches!(sig.return_type, Type::Optional(..)));
}
