use chrono::{DateTime, TimeZone, Utc};

use super::{checkpoint, ProjectionCheckpoint};
use crate::domain::ProjectionDescriptor;

fn descriptor() -> ProjectionDescriptor {
    ProjectionDescriptor {
        name: "logs".to_string(),
        schema_version: 2,
        retention: "7d".to_string(),
    }
}

fn at(millis: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap()
        + chrono::Duration::milliseconds(i64::from(millis))
}

#[test]
fn empty_stamps_the_given_time_with_milliseconds() {
    assert_eq!(
        serde_json::to_string(&ProjectionCheckpoint::empty(&descriptor(), at(0))).unwrap(),
        r#"{"projection":"logs","schema_version":2,"cursor":0,"source_generation":0,"event_id":null,"state_sha256":"","updated_at":"2026-01-02T03:04:05.000Z"}"#
    );
}

#[test]
fn checkpoint_stamps_the_given_time_with_milliseconds() {
    let built = checkpoint(
        &descriptor(),
        42,
        3,
        Some("evt-42".to_string()),
        br#"{"count":1}"#,
        at(678),
    );
    assert_eq!(
        serde_json::to_string(&built).unwrap(),
        r#"{"projection":"logs","schema_version":2,"cursor":42,"source_generation":3,"event_id":"evt-42","state_sha256":"6aea6dfe6561984cdc5c54ead84d47d2cf29e48253ae282aef237404adad4661","updated_at":"2026-01-02T03:04:05.678Z"}"#
    );
}
