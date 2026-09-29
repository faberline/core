use super::*;

fn prior(type_: &str, status: &str, at: &str) -> Condition {
    Condition {
        type_: type_.into(),
        status: status.into(),
        reason: "Whatever".into(),
        message: String::new(),
        last_transition_time: at.into(),
        observed_generation: Some(1),
    }
}

#[test]
fn unchanged_status_keeps_its_original_transition_time() {
    let before = vec![prior("Ready", "True", "2026-01-01T00:00:00Z")];
    let out = project(
        &before,
        vec![ConditionFact::new(
            "Ready",
            ConditionStatus::True,
            "AllReplicasReady",
            "3/3 ready",
        )],
        7,
        "2026-06-06T06:06:06Z",
    );
    assert_eq!(out[0].last_transition_time, "2026-01-01T00:00:00Z");
    // Everything else *does* refresh — only the transition instant is sticky.
    assert_eq!(out[0].reason, "AllReplicasReady");
    assert_eq!(out[0].message, "3/3 ready");
    assert_eq!(out[0].observed_generation, Some(7));
}

#[test]
fn flipped_status_takes_the_injected_time() {
    let before = vec![prior("Ready", "True", "2026-01-01T00:00:00Z")];
    let out = project(
        &before,
        vec![ConditionFact::new(
            "Ready",
            ConditionStatus::False,
            "ReplicasNotReady",
            "1/3 ready",
        )],
        7,
        "2026-06-06T06:06:06Z",
    );
    assert_eq!(out[0].status, "False");
    assert_eq!(out[0].last_transition_time, "2026-06-06T06:06:06Z");
}

#[test]
fn a_condition_seen_for_the_first_time_takes_the_injected_time() {
    let out = project(
        &[],
        vec![ConditionFact::new(
            "Progressing",
            ConditionStatus::Unknown,
            "NoObservation",
            "",
        )],
        0,
        "2026-06-06T06:06:06Z",
    );
    assert_eq!(out[0].last_transition_time, "2026-06-06T06:06:06Z");
    assert_eq!(out[0].status, "Unknown");
}

/// The same prior + facts must project identically no matter how often it
/// runs — this is the property that lets services keep clock-free,
/// deterministic status tests.
#[test]
fn projection_is_deterministic_and_order_preserving() {
    let before = vec![
        prior("Ready", "False", "2026-01-01T00:00:00Z"),
        prior("Progressing", "True", "2026-02-02T00:00:00Z"),
    ];
    let facts = || {
        vec![
            ConditionFact::new("Ready", ConditionStatus::False, "NotReady", "0/3"),
            ConditionFact::new("Progressing", ConditionStatus::True, "Converging", "0/3"),
        ]
    };
    let a = project(&before, facts(), 3, "2026-06-06T06:06:06Z");
    let b = project(&before, facts(), 3, "2026-06-06T06:06:06Z");
    assert_eq!(a, b);
    assert_eq!(a[0].type_, "Ready");
    assert_eq!(a[1].type_, "Progressing");
}

/// A condition that reappears after being dropped is a *new* condition —
/// nothing to carry forward, so it takes the injected time.
#[test]
fn dropped_conditions_do_not_resurrect_their_old_transition_time() {
    let before = vec![prior("ReshardInProgress", "True", "2026-01-01T00:00:00Z")];
    let dropped = project(&before, vec![], 1, "2026-03-03T00:00:00Z");
    assert!(dropped.is_empty());
    let back = project(
        &dropped,
        vec![ConditionFact::new(
            "ReshardInProgress",
            ConditionStatus::True,
            "Splitting",
            "",
        )],
        2,
        "2026-04-04T00:00:00Z",
    );
    assert_eq!(back[0].last_transition_time, "2026-04-04T00:00:00Z");
}

#[test]
fn serialized_shape_is_metav1_condition() {
    let out = project(
        &[],
        vec![ConditionFact::new(
            "Ready",
            ConditionStatus::True,
            "AllReplicasReady",
            "3/3 ready",
        )],
        9,
        "2026-06-06T06:06:06Z",
    );
    assert_eq!(
        serde_json::to_value(&out[0]).expect("serialize"),
        serde_json::json!({
            "type": "Ready",
            "status": "True",
            "reason": "AllReplicasReady",
            "message": "3/3 ready",
            "lastTransitionTime": "2026-06-06T06:06:06Z",
            "observedGeneration": 9,
        })
    );
}

#[test]
fn now_rfc3339_has_no_subsecond_component() {
    let now = now_rfc3339();
    assert!(now.ends_with('Z'), "{now} is not UTC-suffixed");
    assert!(!now.contains('.'), "{now} carries sub-second precision");
}
