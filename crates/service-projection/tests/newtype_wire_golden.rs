use service_projection::{
    ProjectionCheckpoint, ProjectionCursor, ProjectionDescriptor, ProjectionLag, SourceGeneration,
};
use utoipa::ToSchema;

#[test]
fn projection_openapi_schemas_keep_the_p2_baseline_bytes() {
    let schemas = [
        ProjectionDescriptor::schema(),
        ProjectionCheckpoint::schema(),
        ProjectionLag::schema(),
    ];
    assert_eq!(
        serde_json::to_string(&schemas).unwrap(),
        include_str!("fixtures/newtype_schemas.json").trim_end()
    );
}

#[test]
fn cursor_and_generation_keep_extreme_wire_values() {
    for value in [0, u64::MAX] {
        let json = value.to_string();
        let cursor = ProjectionCursor::new(value);
        let generation = SourceGeneration::new(value);
        assert_eq!(serde_json::to_string(&cursor).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<ProjectionCursor>(&json).unwrap(),
            cursor
        );
        assert_eq!(serde_json::to_string(&generation).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<SourceGeneration>(&json).unwrap(),
            generation
        );
    }
    assert_eq!(
        ProjectionCursor::new(3).distance_since(ProjectionCursor::new(5)),
        0
    );
    assert_eq!(
        ProjectionCursor::new(8).distance_since(ProjectionCursor::new(5)),
        3
    );
}
