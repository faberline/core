use super::*;
use crate::type_inference::Type;

#[test]
fn test_model_builder() {
    let mut model = ModelDef::new("User").with_description("User account model");

    model.add_field(FieldDef::new("id", Type::Int).primary_key());
    model.add_field(FieldDef::new("email", Type::Str));
    model.add_field(
        FieldDef::new("name", Type::Str)
            .optional()
            .with_description("Display name"),
    );

    assert_eq!(model.name, "User");
    assert_eq!(model.fields.len(), 3);
    assert!(model.fields[0].primary_key);
    assert!(!model.fields[2].required);
}

#[test]
fn test_data_model_spec() {
    let mut spec = DataModelSpec::new();

    let user = ModelDef::new("User");
    let post = ModelDef::new("Post");

    spec.add_model(user);
    spec.add_model(post);

    spec.add_relationship(Relationship {
        from_model: "Post".into(),
        from_field: "author_id".into(),
        to_model: "User".into(),
        to_field: "id".into(),
        rel_type: RelationType::ManyToOne,
    });

    assert_eq!(spec.models.len(), 2);
    assert!(spec.get_model("User").is_some());
    assert!(spec.get_model("Post").is_some());
}
