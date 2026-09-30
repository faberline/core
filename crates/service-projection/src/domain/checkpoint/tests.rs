use super::ProjectionCheckpoint;

const CHECKPOINT_JSON: &str = r#"{"projection":"logs","schema_version":2,"cursor":42,"source_generation":3,"event_id":"evt-42","state_sha256":"6aea6dfe6561984cdc5c54ead84d47d2cf29e48253ae282aef237404adad4661","updated_at":"2026-01-02T03:04:05.678Z"}"#;

const EMPTY_CHECKPOINT_JSON: &str = r#"{"projection":"logs","schema_version":2,"cursor":0,"source_generation":0,"event_id":null,"state_sha256":"","updated_at":"2026-01-02T03:04:05.000Z"}"#;

/// Checkpoints written before `source_generation` existed decode as
/// generation 0.
const PRE_GENERATION_CHECKPOINT_JSON: &str = r#"{"projection":"logs","schema_version":2,"cursor":0,"event_id":null,"state_sha256":"","updated_at":"2026-01-02T03:04:05.000Z"}"#;

fn checkpoint() -> ProjectionCheckpoint {
    ProjectionCheckpoint {
        projection: "logs".to_string(),
        schema_version: 2,
        cursor: 42,
        source_generation: 3,
        event_id: Some("evt-42".to_string()),
        state_sha256: "6aea6dfe6561984cdc5c54ead84d47d2cf29e48253ae282aef237404adad4661"
            .to_string(),
        updated_at: "2026-01-02T03:04:05.678Z".to_string(),
    }
}

fn empty_checkpoint() -> ProjectionCheckpoint {
    ProjectionCheckpoint {
        projection: "logs".to_string(),
        schema_version: 2,
        cursor: 0,
        source_generation: 0,
        event_id: None,
        state_sha256: String::new(),
        updated_at: "2026-01-02T03:04:05.000Z".to_string(),
    }
}

#[test]
fn projection_checkpoint_json_is_pinned() {
    assert_eq!(
        serde_json::to_string(&checkpoint()).unwrap(),
        CHECKPOINT_JSON
    );
    assert_eq!(
        serde_json::from_str::<ProjectionCheckpoint>(CHECKPOINT_JSON).unwrap(),
        checkpoint()
    );
}

#[test]
fn empty_projection_checkpoint_json_is_pinned() {
    assert_eq!(
        serde_json::to_string(&empty_checkpoint()).unwrap(),
        EMPTY_CHECKPOINT_JSON
    );
    assert_eq!(
        serde_json::from_str::<ProjectionCheckpoint>(EMPTY_CHECKPOINT_JSON).unwrap(),
        empty_checkpoint()
    );
    assert_eq!(
        serde_json::from_str::<ProjectionCheckpoint>(PRE_GENERATION_CHECKPOINT_JSON).unwrap(),
        empty_checkpoint()
    );
}
