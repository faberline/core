use super::*;

fn job(name: &str, phase: BackupJobPhase) -> ObservedBackupJob {
    ObservedBackupJob {
        name: name.to_string(),
        phase,
    }
}

#[test]
fn name_joins_service_and_slot() {
    assert_eq!(
        backup_job_name("sample", "20261108t0200z").unwrap(),
        "sample-backup-20261108t0200z"
    );
}

#[test]
fn name_longer_than_63_chars_is_rejected() {
    let service = "a".repeat(50);
    let slot = "x".repeat(10);
    assert!(matches!(
        backup_job_name(&service, &slot),
        Err(BackupJobError::NameTooLong { .. })
    ));
}

#[test]
fn bad_slots_are_rejected() {
    for slot in ["", "A", "-a", "a-", "a_b", &"a".repeat(41)] {
        assert!(
            matches!(
                backup_job_name("sample", slot),
                Err(BackupJobError::InvalidSlot(_))
            ),
            "slot {slot:?} must be rejected"
        );
    }
}

#[test]
fn empty_observation_starts_the_slot_job() {
    assert_eq!(
        plan_backup_job("sample", "s1", &[]).unwrap(),
        BackupJobDecision::Start {
            job_name: "sample-backup-s1".into()
        }
    );
}

#[test]
fn active_job_blocks_a_new_start() {
    let observed = [job("sample-backup-s0", BackupJobPhase::Active)];
    assert_eq!(
        plan_backup_job("sample", "s1", &observed).unwrap(),
        BackupJobDecision::WaitForActive {
            job_name: "sample-backup-s0".into()
        }
    );
}

#[test]
fn smallest_active_job_name_is_reported() {
    let observed = [
        job("sample-backup-s2", BackupJobPhase::Active),
        job("sample-backup-s0", BackupJobPhase::Active),
    ];
    assert_eq!(
        plan_backup_job("sample", "s1", &observed).unwrap(),
        BackupJobDecision::WaitForActive {
            job_name: "sample-backup-s0".into()
        }
    );
}

#[test]
fn active_job_of_the_same_slot_waits_not_already_ran() {
    let observed = [job("sample-backup-s1", BackupJobPhase::Active)];
    assert_eq!(
        plan_backup_job("sample", "s1", &observed).unwrap(),
        BackupJobDecision::WaitForActive {
            job_name: "sample-backup-s1".into()
        }
    );
}

#[test]
fn two_concurrent_reconciles_choose_the_same_job_name() {
    let first = plan_backup_job("sample", "s1", &[]).unwrap();
    let second = plan_backup_job("sample", "s1", &[]).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first,
        BackupJobDecision::Start {
            job_name: "sample-backup-s1".into()
        }
    );
}

#[test]
fn finished_job_of_the_same_slot_is_not_started_again() {
    for phase in [BackupJobPhase::Succeeded, BackupJobPhase::Failed] {
        let observed = [job("sample-backup-s1", phase)];
        assert_eq!(
            plan_backup_job("sample", "s1", &observed).unwrap(),
            BackupJobDecision::AlreadyRan {
                job_name: "sample-backup-s1".into()
            }
        );
    }
}

#[test]
fn finished_job_of_an_older_slot_does_not_block() {
    let observed = [job("sample-backup-s0", BackupJobPhase::Succeeded)];
    assert_eq!(
        plan_backup_job("sample", "s1", &observed).unwrap(),
        BackupJobDecision::Start {
            job_name: "sample-backup-s1".into()
        }
    );
}

#[test]
fn library_has_no_implicit_policy() {
    assert!(!include_str!("../backup_job.rs").contains("Default"));
}
