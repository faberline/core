use super::*;

#[test]
fn no_token_exists_before_assignment() {
    let state = FencedAssignment::idle();
    assert_eq!(state.epoch(), AssignmentEpoch::new(0));
    assert_eq!(state.token(), None);
}

#[test]
fn assignment_is_exclusive_until_explicit_release_or_expiry() {
    let mut state = FencedAssignment::idle();
    let token = state.assign(NodeId::new(1), 10, 20).unwrap();
    assert_eq!(
        token,
        FenceToken {
            owner: NodeId::new(1),
            epoch: AssignmentEpoch::new(1)
        }
    );
    assert!(matches!(
        state.assign(NodeId::new(2), 21, 30),
        Err(AssignmentError::AlreadyAssigned(_))
    ));
    assert!(matches!(
        state.expire(19),
        Err(AssignmentError::NotExpired { .. })
    ));
    state.expire(20).unwrap();
    let next = state.assign(NodeId::new(2), 20, 30).unwrap();
    assert_eq!(
        next,
        FenceToken {
            owner: NodeId::new(2),
            epoch: AssignmentEpoch::new(2)
        }
    );
}

#[test]
fn stale_owner_is_rejected_after_reassignment() {
    let mut state = FencedAssignment::idle();
    let old = state.assign(NodeId::new(1), 0, 10).unwrap();
    state.expire(10).unwrap();
    let current = state.assign(NodeId::new(2), 10, 20).unwrap();
    assert!(matches!(
        state.validate(old, 11),
        Err(AssignmentError::StaleEpoch { current, provided })
            if current == AssignmentEpoch::new(2) && provided == AssignmentEpoch::new(1)
    ));
    assert_eq!(state.validate(current, 11).unwrap().token, current);
}

#[test]
fn renewal_requires_current_owner_epoch_and_later_expiry() {
    let mut state = FencedAssignment::idle();
    let token = state.assign(NodeId::new(3), 100, 200).unwrap();
    assert!(matches!(
        state.renew(
            FenceToken {
                owner: NodeId::new(4),
                ..token
            },
            150,
            250
        ),
        Err(AssignmentError::OwnerMismatch { .. })
    ));
    assert!(matches!(
        state.renew(token, 150, 200),
        Err(AssignmentError::ExpiryNotExtended { .. })
    ));
    assert_eq!(state.renew(token, 150, 250).unwrap().expires_at_ms, 250);
}

#[test]
fn release_retains_epoch_and_fences_late_completion() {
    let mut state = FencedAssignment::idle();
    let token = state.assign(NodeId::new(5), 0, 100).unwrap();
    state.release(token, 50).unwrap();
    assert_eq!(state.epoch(), AssignmentEpoch::new(1));
    assert!(matches!(
        state.validate(token, 51),
        Err(AssignmentError::Unassigned { current_epoch })
            if current_epoch == AssignmentEpoch::new(1)
    ));
    assert_eq!(
        state.assign(NodeId::new(5), 51, 100).unwrap().epoch,
        AssignmentEpoch::new(2)
    );
}

#[test]
fn identical_commands_produce_identical_replica_state() {
    let mut a = FencedAssignment::idle();
    let mut b = FencedAssignment::idle();
    for state in [&mut a, &mut b] {
        let first = state.assign(NodeId::new(1), 1_000, 2_000).unwrap();
        state.renew(first, 1_500, 2_500).unwrap();
        state.expire(2_500).unwrap();
        state.assign(NodeId::new(2), 2_500, 3_500).unwrap();
    }
    assert_eq!(a, b);
    let bytes = serde_json::to_vec(&a).unwrap();
    assert_eq!(
        serde_json::from_slice::<FencedAssignment>(&bytes).unwrap(),
        a
    );
}

#[test]
fn epoch_exhaustion_is_rejected() {
    let mut state = FencedAssignment {
        epoch: AssignmentEpoch::new(u64::MAX),
        active: None,
    };
    assert_eq!(
        state.assign(NodeId::new(1), 0, 10),
        Err(AssignmentError::EpochExhausted)
    );
}

#[test]
fn epoch_prints_and_serializes_as_the_bare_number() {
    let epoch = AssignmentEpoch::new(7);
    assert_eq!(format!("{epoch} {epoch:?}"), "7 7");
    assert_eq!(serde_json::to_string(&epoch).unwrap(), "7");
    assert_eq!(serde_json::from_str::<AssignmentEpoch>("7").unwrap(), epoch);
}
