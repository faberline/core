use super::*;

fn request(id: &str) -> RestoreRequest {
    RestoreRequest {
        restore_id: id.to_string(),
        manifest: BackupManifestRef {
            id: "m1".to_string(),
            location: "gs://bucket/m1".to_string(),
        },
    }
}

fn job(name: &str, phase: RestoreJobPhase) -> ObservedRestoreJob {
    ObservedRestoreJob {
        name: name.to_string(),
        phase,
    }
}

fn status(id: &str, phase: RestorePhase) -> RestoreStatus {
    RestoreStatus {
        restore_id: id.to_string(),
        manifest: request(id).manifest,
        phase,
    }
}

#[test]
fn name_joins_service_and_restore_id() {
    assert_eq!(
        restore_job_name("sample", "r1").unwrap(),
        "sample-restore-r1"
    );
}

#[test]
fn bad_restore_ids_are_rejected() {
    for id in ["", "A", "-a", "a-", "a_b", &"a".repeat(41)] {
        assert!(
            matches!(
                restore_job_name("sample", id),
                Err(RestoreJobError::InvalidRestoreId(_))
            ),
            "id {id:?} must be rejected"
        );
    }
}

#[test]
fn name_longer_than_63_chars_is_rejected() {
    let service = "a".repeat(50);
    let id = "x".repeat(10);
    assert!(matches!(
        restore_job_name(&service, &id),
        Err(RestoreJobError::NameTooLong { .. })
    ));
}

#[test]
fn unverified_manifest_is_rejected() {
    let empty_id = RestoreRequest {
        restore_id: "r1".to_string(),
        manifest: BackupManifestRef {
            id: String::new(),
            location: "gs://b/m".to_string(),
        },
    };
    let no_scheme = RestoreRequest {
        restore_id: "r1".to_string(),
        manifest: BackupManifestRef {
            id: "m1".to_string(),
            location: "nope".to_string(),
        },
    };
    assert_eq!(
        plan_restore_job("sample", &empty_id, &[], None),
        Err(RestoreJobError::UnverifiedManifest)
    );
    assert_eq!(
        plan_restore_job("sample", &no_scheme, &[], None),
        Err(RestoreJobError::UnverifiedManifest)
    );
}

#[test]
fn empty_observation_starts_the_job() {
    assert_eq!(
        plan_restore_job("sample", &request("r1"), &[], None),
        Ok(RestoreJobDecision::Start {
            job_name: "sample-restore-r1".to_string()
        })
    );
}

#[test]
fn active_job_blocks_a_second_start() {
    let observed = [job("sample-restore-r0", RestoreJobPhase::Active)];
    assert_eq!(
        plan_restore_job("sample", &request("r1"), &observed, None),
        Ok(RestoreJobDecision::WaitForActive {
            job_name: "sample-restore-r0".to_string()
        })
    );
}

#[test]
fn existing_finished_job_is_not_started_again() {
    for phase in [RestoreJobPhase::Succeeded, RestoreJobPhase::Failed] {
        let observed = [job("sample-restore-r1", phase)];
        assert_eq!(
            plan_restore_job("sample", &request("r1"), &observed, None),
            Ok(RestoreJobDecision::AlreadyRan {
                job_name: "sample-restore-r1".to_string()
            })
        );
    }
}

#[test]
fn completed_status_blocks_a_start_after_the_job_is_cleaned_up() {
    let complete = status("r1", RestorePhase::Complete);
    assert_eq!(
        plan_restore_job("sample", &request("r1"), &[], Some(&complete)),
        Ok(RestoreJobDecision::AlreadyRan {
            job_name: "sample-restore-r1".to_string()
        })
    );
}

#[test]
fn completed_status_of_another_restore_id_does_not_block() {
    let complete_r0 = status("r0", RestorePhase::Complete);
    assert_eq!(
        plan_restore_job("sample", &request("r1"), &[], Some(&complete_r0)),
        Ok(RestoreJobDecision::Start {
            job_name: "sample-restore-r1".to_string()
        })
    );
}

#[test]
fn status_follows_the_job_outcome() {
    let req = request("r1");
    assert_eq!(
        next_restore_status(None, &req, &RestoreJobOutcome::NotStarted, false).phase,
        RestorePhase::Pending
    );
    assert_eq!(
        next_restore_status(None, &req, &RestoreJobOutcome::Active, false).phase,
        RestorePhase::Running
    );
    assert_eq!(
        next_restore_status(
            None,
            &req,
            &RestoreJobOutcome::Failed {
                reason: "boom".to_string()
            },
            false
        )
        .phase,
        RestorePhase::Failed {
            reason: "boom".to_string()
        }
    );
}

#[test]
fn success_without_ack_waits_for_the_service() {
    let req = request("r1");
    assert_eq!(
        next_restore_status(None, &req, &RestoreJobOutcome::Succeeded, false).phase,
        RestorePhase::AwaitingAck
    );
    assert_eq!(
        next_restore_status(None, &req, &RestoreJobOutcome::Succeeded, true).phase,
        RestorePhase::Complete
    );
}

#[test]
fn ack_is_ignored_unless_the_job_succeeded() {
    let req = request("r1");
    assert_eq!(
        next_restore_status(None, &req, &RestoreJobOutcome::Active, true).phase,
        RestorePhase::Running
    );
    assert_eq!(
        next_restore_status(
            None,
            &req,
            &RestoreJobOutcome::Failed {
                reason: "boom".to_string()
            },
            true
        )
        .phase,
        RestorePhase::Failed {
            reason: "boom".to_string()
        }
    );
}

#[test]
fn complete_is_terminal() {
    let req = request("r1");
    let previous = status("r1", RestorePhase::Complete);
    assert_eq!(
        next_restore_status(Some(&previous), &req, &RestoreJobOutcome::Active, false),
        previous
    );
}

#[test]
fn readiness_is_gated_until_complete() {
    assert!(!restore_gates_readiness(None));
    assert!(restore_gates_readiness(Some(&status(
        "r1",
        RestorePhase::Pending
    ))));
    assert!(restore_gates_readiness(Some(&status(
        "r1",
        RestorePhase::Running
    ))));
    assert!(restore_gates_readiness(Some(&status(
        "r1",
        RestorePhase::Failed {
            reason: "boom".to_string()
        }
    ))));
    assert!(restore_gates_readiness(Some(&status(
        "r1",
        RestorePhase::AwaitingAck
    ))));
    assert!(!restore_gates_readiness(Some(&status(
        "r1",
        RestorePhase::Complete
    ))));
}

#[test]
fn finalizer_is_removable_only_after_ack_or_failure() {
    assert!(restore_finalizer_removable(&status(
        "r1",
        RestorePhase::Complete
    )));
    assert!(restore_finalizer_removable(&status(
        "r1",
        RestorePhase::Failed {
            reason: "boom".to_string()
        }
    )));
    assert!(!restore_finalizer_removable(&status(
        "r1",
        RestorePhase::Pending
    )));
    assert!(!restore_finalizer_removable(&status(
        "r1",
        RestorePhase::Running
    )));
    assert!(!restore_finalizer_removable(&status(
        "r1",
        RestorePhase::AwaitingAck
    )));
}

#[test]
fn restart_at_every_phase_resumes_once() {
    let req = request("r1");
    let steps: [Vec<ObservedRestoreJob>; 3] = [
        vec![],
        vec![job("sample-restore-r1", RestoreJobPhase::Active)],
        vec![job("sample-restore-r1", RestoreJobPhase::Succeeded)],
    ];
    let outcomes = [
        RestoreJobOutcome::NotStarted,
        RestoreJobOutcome::Active,
        RestoreJobOutcome::Succeeded,
    ];
    let mut starts = 0;
    let mut status_now: Option<RestoreStatus> = None;
    for (observed, outcome) in steps.iter().zip(outcomes.iter()) {
        let first = plan_restore_job("sample", &req, observed, status_now.as_ref()).unwrap();
        let again = plan_restore_job("sample", &req, observed, status_now.as_ref()).unwrap();
        assert_eq!(first, again, "a restart must give the same decision");
        if matches!(first, RestoreJobDecision::Start { .. }) {
            starts += 1;
        }
        status_now = Some(next_restore_status(
            status_now.as_ref(),
            &req,
            outcome,
            false,
        ));
    }
    assert_eq!(starts, 1, "the restore Job must start exactly once");
}

#[test]
fn library_has_no_implicit_policy() {
    assert!(!include_str!("../restore_job.rs").contains("Default"));
}
