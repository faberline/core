use super::ProjectionDescriptor;

const DESCRIPTOR_JSON: &str = r#"{"name":"logs","schema_version":2,"retention":"7d"}"#;

fn descriptor() -> ProjectionDescriptor {
    ProjectionDescriptor {
        name: "logs".to_string(),
        schema_version: 2,
        retention: "7d".to_string(),
    }
}

#[test]
fn projection_descriptor_json_is_pinned() {
    assert_eq!(
        serde_json::to_string(&descriptor()).unwrap(),
        DESCRIPTOR_JSON
    );
    assert_eq!(
        serde_json::from_str::<ProjectionDescriptor>(DESCRIPTOR_JSON).unwrap(),
        descriptor()
    );
}

#[test]
fn try_new_keeps_the_fields_and_rejects_invalid_names() {
    let built = ProjectionDescriptor::try_new("logs", 2, "7d").unwrap();
    assert_eq!(built, descriptor());
    assert_eq!(
        (built.name(), built.schema_version(), built.retention()),
        ("logs", 2, "7d")
    );
    for name in ["", "  ", "a/b", "a\0b"] {
        assert!(matches!(
            ProjectionDescriptor::try_new(name, 1, "7d"),
            Err(super::ProjectionError::InvalidName)
        ));
    }
}
