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
