use super::{StagingMarker, StagingState};
use crate::domain::SegmentError;

const MARKER_JSON: &str =
    r#"{"format_version":1,"segment_key":"archive/segment-0001.parquet","state":"intent"}"#;

const ALL_STATES: [StagingState; 4] = [
    StagingState::Intent,
    StagingState::Uploaded,
    StagingState::Committed,
    StagingState::GcPending,
];

const LEGAL: [(StagingState, StagingState); 4] = [
    (StagingState::Intent, StagingState::Uploaded),
    (StagingState::Uploaded, StagingState::Committed),
    (StagingState::Intent, StagingState::GcPending),
    (StagingState::Uploaded, StagingState::GcPending),
];

fn marker(state: StagingState) -> StagingMarker {
    StagingMarker {
        format_version: 1,
        segment_key: "a".to_string(),
        state,
    }
}

#[test]
fn marker_json_is_pinned() {
    let fresh = StagingMarker::new("archive/segment-0001.parquet");
    assert_eq!(fresh.encode().unwrap(), MARKER_JSON.as_bytes());
    assert_eq!(
        StagingMarker::decode(MARKER_JSON.as_bytes()).unwrap(),
        fresh
    );
}

#[test]
fn state_names_are_pinned() {
    assert_eq!(
        serde_json::to_string(&StagingState::Intent).unwrap(),
        "\"intent\""
    );
    assert_eq!(
        serde_json::to_string(&StagingState::Uploaded).unwrap(),
        "\"uploaded\""
    );
    assert_eq!(
        serde_json::to_string(&StagingState::Committed).unwrap(),
        "\"committed\""
    );
    assert_eq!(
        serde_json::to_string(&StagingState::GcPending).unwrap(),
        "\"gc_pending\""
    );
}

#[test]
fn object_key_is_prefix_plus_hash_of_the_segment_key() {
    assert_eq!(
        StagingMarker::new("archive/segment-0001.parquet").object_key(),
        "_archive/stage/fb6c3989579d2eae9aff55014178ae1764a8ef8b1ad93c0294d46d8ce7fac5b2.json"
    );
}

#[test]
fn legal_transitions_are_accepted() {
    for (from, to) in LEGAL {
        let moved = marker(from).move_to(to).unwrap();
        assert_eq!(moved.state, to);
        assert_eq!(moved.segment_key, "a");
    }
}

#[test]
fn committed_to_gc_pending_is_refused() {
    assert!(matches!(
        marker(StagingState::Committed).move_to(StagingState::GcPending),
        Err(SegmentError::IllegalStagingTransition {
            from: StagingState::Committed,
            to: StagingState::GcPending,
        })
    ));
}

#[test]
fn every_other_pair_is_refused() {
    let mut allowed = 0;
    for from in ALL_STATES {
        for to in ALL_STATES {
            let expected = LEGAL.contains(&(from, to));
            assert_eq!(from.can_move_to(to), expected, "{from:?} to {to:?}");
            if expected {
                allowed += 1;
            }
        }
    }
    assert_eq!(allowed, 4);
}

#[test]
fn decode_refuses_a_wrong_version() {
    let text = MARKER_JSON.replace("\"format_version\":1", "\"format_version\":2");
    assert!(matches!(
        StagingMarker::decode(text.as_bytes()),
        Err(SegmentError::UnsupportedStagingMarkerVersion { version: 2 })
    ));
}

#[test]
fn decode_refuses_a_truncated_record() {
    assert!(matches!(
        StagingMarker::decode(&MARKER_JSON.as_bytes()[..40]),
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
