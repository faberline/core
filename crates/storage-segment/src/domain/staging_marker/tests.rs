use super::{StagingMarker, StagingState};
use crate::domain::SegmentError;

const SEGMENT: &str = "archive/segment-0001.parquet";
const MARKER_JSON: &str =
    r#"{"format_version":1,"segment_key":"archive/segment-0001.parquet","state":"intent"}"#;

fn marker_in(state: StagingState) -> StagingMarker {
    StagingMarker {
        format_version: 1,
        segment_key: "a".into(),
        state,
    }
}

#[test]
fn marker_json_is_pinned() {
    assert_eq!(
        StagingMarker::new(SEGMENT).encode().unwrap(),
        MARKER_JSON.as_bytes()
    );
    assert_eq!(
        StagingMarker::decode(MARKER_JSON.as_bytes()).unwrap(),
        StagingMarker::new(SEGMENT)
    );
}

#[test]
fn state_names_are_pinned() {
    let name = |state: StagingState| serde_json::to_string(&state).unwrap();
    assert_eq!(name(StagingState::Intent), "\"intent\"");
    assert_eq!(name(StagingState::Uploaded), "\"uploaded\"");
    assert_eq!(name(StagingState::Committed), "\"committed\"");
    assert_eq!(name(StagingState::GcPending), "\"gc_pending\"");
}

#[test]
fn object_key_is_prefix_plus_hash_of_the_segment_key() {
    assert_eq!(
        StagingMarker::new(SEGMENT).object_key(),
        "_archive/stage/fb6c3989579d2eae9aff55014178ae1764a8ef8b1ad93c0294d46d8ce7fac5b2.json"
    );
}

#[test]
fn legal_transitions_are_accepted() {
    let cases = [
        (StagingState::Intent, StagingState::Uploaded),
        (StagingState::Uploaded, StagingState::Committed),
        (StagingState::Intent, StagingState::GcPending),
        (StagingState::Uploaded, StagingState::GcPending),
    ];
    for (from, to) in cases {
        assert_eq!(marker_in(from).move_to(to).unwrap().state, to);
    }
}

#[test]
fn committed_to_gc_pending_is_refused() {
    let result = marker_in(StagingState::Committed).move_to(StagingState::GcPending);
    assert!(matches!(
        result,
        Err(SegmentError::IllegalStagingTransition {
            from: StagingState::Committed,
            to: StagingState::GcPending
        })
    ));
}

#[test]
fn every_other_pair_is_refused() {
    use StagingState::*;
    let all = [Intent, Uploaded, Committed, GcPending];
    let legal = [
        (Intent, Uploaded),
        (Uploaded, Committed),
        (Intent, GcPending),
        (Uploaded, GcPending),
    ];
    let mut accepted = 0;
    for from in all {
        for to in all {
            let expected = legal.contains(&(from, to));
            assert_eq!(from.can_move_to(to), expected, "{from:?} to {to:?}");
            if expected {
                accepted += 1;
            }
        }
    }
    assert_eq!(accepted, 4);
}

#[test]
fn decode_refuses_a_wrong_version() {
    let json = MARKER_JSON.replace("\"format_version\":1", "\"format_version\":2");
    assert!(matches!(
        StagingMarker::decode(json.as_bytes()),
        Err(SegmentError::UnsupportedStagingMarkerVersion { version: 2 })
    ));
}

#[test]
fn decode_refuses_a_truncated_record() {
    let bytes = MARKER_JSON.as_bytes();
    assert!(matches!(
        StagingMarker::decode(&bytes[..40]),
        Err(SegmentError::CorruptStagingMarker { .. })
    ));
    assert!(matches!(
        StagingMarker::decode(b""),
        Err(SegmentError::CorruptStagingMarker { .. })
    ));
    assert!(matches!(
        StagingMarker::decode(br#"{"segment_key":"a"}"#),
        Err(SegmentError::CorruptStagingMarker { .. })
    ));
}
