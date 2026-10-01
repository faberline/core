use super::{persist, ProjectionStateEnvelope, PROJECTION_STATE_FORMAT_VERSION};
use crate::domain::ProjectionCheckpoint;
use crate::{ProjectionCursor, ProjectionEventId, ProjectionName, SourceGeneration};

/// The state file bytes: pretty JSON with two-space indentation and no
/// trailing newline.
const ENVELOPE_JSON: &str = r#"{
  "format_version": 1,
  "checkpoint": {
    "projection": "logs",
    "schema_version": 2,
    "cursor": 42,
    "source_generation": 3,
    "event_id": "evt-42",
    "state_sha256": "6aea6dfe6561984cdc5c54ead84d47d2cf29e48253ae282aef237404adad4661",
    "updated_at": "2026-01-02T03:04:05.678Z"
  },
  "state_encoding": "base64",
  "state_base64": "eyJjb3VudCI6MX0="
}"#;

const STATE: &[u8] = br#"{"count":1}"#;

fn checkpoint() -> ProjectionCheckpoint {
    ProjectionCheckpoint {
        projection: ProjectionName::new("logs"),
        schema_version: 2,
        cursor: ProjectionCursor::new(42),
        source_generation: SourceGeneration::new(3),
        event_id: Some(ProjectionEventId::new("evt-42")),
        state_sha256: "6aea6dfe6561984cdc5c54ead84d47d2cf29e48253ae282aef237404adad4661"
            .to_string(),
        updated_at: "2026-01-02T03:04:05.678Z".to_string(),
    }
}

fn envelope() -> ProjectionStateEnvelope {
    ProjectionStateEnvelope {
        format_version: PROJECTION_STATE_FORMAT_VERSION,
        checkpoint: checkpoint(),
        state_encoding: "base64".to_string(),
        state_base64: "eyJjb3VudCI6MX0=".to_string(),
    }
}

#[test]
fn projection_state_envelope_json_is_pinned() {
    assert_eq!(
        serde_json::to_string_pretty(&envelope()).unwrap(),
        ENVELOPE_JSON
    );
    assert_eq!(
        serde_json::from_str::<ProjectionStateEnvelope>(ENVELOPE_JSON).unwrap(),
        envelope()
    );
}

#[test]
fn persisted_state_file_bytes_are_pinned() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("logs").join("state.json");
    persist(&path, &checkpoint(), STATE).unwrap();
    assert_eq!(
        String::from_utf8(std::fs::read(&path).unwrap()).unwrap(),
        ENVELOPE_JSON
    );
}
