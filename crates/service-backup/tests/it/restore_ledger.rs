use std::collections::HashMap;

use serde_json::json;
use service_backup::{
    resume_or_begin, ArtifactStage, RestoreLedger, RestoreLedgerError, RestorePhase, ResumePoint,
    MAX_RESTORE_LEDGER_ARTIFACTS, MAX_RESTORE_LEDGER_BYTES,
};

const VALID_SET_ID: &str = "backup-set-20261024.001";
const VALID_DIGEST: &str =
    "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ALT_DIGEST: &str = "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn sample_artifacts() -> Vec<(String, u64)> {
    vec![
        ("data/part-0.bin".to_string(), 1024),
        ("data/part-1.bin".to_string(), 2048),
        ("data/part-2.bin".to_string(), 4096),
    ]
}

#[test]
fn valid_full_path() {
    let artifacts = sample_artifacts();
    let mut ledger = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &artifacts)
        .expect("valid begin should succeed");

    assert_eq!(ledger.phase(), RestorePhase::Staging);
    assert_eq!(ledger.set_id(), VALID_SET_ID);
    assert_eq!(ledger.manifest_digest(), VALID_DIGEST);
    assert_eq!(
        ledger.pending(),
        vec!["data/part-0.bin", "data/part-1.bin", "data/part-2.bin"]
    );
    assert_eq!(
        ledger.unapplied(),
        vec!["data/part-0.bin", "data/part-1.bin", "data/part-2.bin"]
    );

    // Verify artifacts one by one
    ledger.mark_verified("data/part-0.bin").unwrap();
    assert_eq!(ledger.pending(), vec!["data/part-1.bin", "data/part-2.bin"]);
    assert_eq!(
        ledger.unapplied(),
        vec!["data/part-0.bin", "data/part-1.bin", "data/part-2.bin"]
    );

    ledger.mark_verified("data/part-1.bin").unwrap();
    ledger.mark_verified("data/part-2.bin").unwrap();
    assert!(ledger.pending().is_empty());
    assert_eq!(
        ledger.unapplied(),
        vec!["data/part-0.bin", "data/part-1.bin", "data/part-2.bin"]
    );

    // Begin apply
    ledger.begin_apply().unwrap();
    assert_eq!(ledger.phase(), RestorePhase::Applying);

    // Apply artifacts one by one
    ledger.mark_applied("data/part-0.bin").unwrap();
    assert_eq!(
        ledger.unapplied(),
        vec!["data/part-1.bin", "data/part-2.bin"]
    );

    ledger.mark_applied("data/part-1.bin").unwrap();
    ledger.mark_applied("data/part-2.bin").unwrap();
    assert!(ledger.unapplied().is_empty());

    // Commit
    ledger.commit().unwrap();
    assert_eq!(ledger.phase(), RestorePhase::Committed);
    assert!(ledger.pending().is_empty());
    assert!(ledger.unapplied().is_empty());
}

#[test]
fn each_invalid_begin_input() {
    let artifacts = sample_artifacts();

    // 1. Empty set_id
    let err = RestoreLedger::begin("", VALID_DIGEST, &artifacts).unwrap_err();
    assert!(matches!(err, RestoreLedgerError::InvalidSetId(s) if s.is_empty()));

    // 2. Oversized set_id (> 128 bytes)
    let long_id = "a".repeat(129);
    let err = RestoreLedger::begin(&long_id, VALID_DIGEST, &artifacts).unwrap_err();
    assert!(matches!(err, RestoreLedgerError::InvalidSetId(s) if s == long_id));

    // 3. Invalid characters in set_id
    for bad_id in &["set id", "set@1", "set:1", "set/1", "set#1", "set$1"] {
        let err = RestoreLedger::begin(bad_id, VALID_DIGEST, &artifacts).unwrap_err();
        assert!(matches!(err, RestoreLedgerError::InvalidSetId(_)));
    }

    // 4. Invalid manifest digests
    for bad_digest in &[
        "sha512:abc",
        "sha256:",
        "sha256:123",
        &format!("sha256:{}", "A".repeat(64)), // uppercase hex forbidden
        &format!("sha256:{}z", "0".repeat(63)), // invalid non-hex char
        &format!("sha256:{}", "0".repeat(65)), // 65 chars
    ] {
        let err = RestoreLedger::begin(VALID_SET_ID, bad_digest, &artifacts).unwrap_err();
        assert!(matches!(err, RestoreLedgerError::InvalidManifestDigest(_)));
    }

    // 5. Empty artifacts list
    let err = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &[]).unwrap_err();
    assert!(matches!(err, RestoreLedgerError::EmptyArtifacts));

    // 6. Oversized artifacts (> 1024)
    let too_many: Vec<(String, u64)> = (0..=MAX_RESTORE_LEDGER_ARTIFACTS)
        .map(|i| (format!("part-{i}.bin"), 10))
        .collect();
    let err = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &too_many).unwrap_err();
    assert!(matches!(
        err,
        RestoreLedgerError::TooManyArtifacts {
            count: 1025,
            max: 1024
        }
    ));

    // 7. Empty artifact name
    let empty_name = vec![("".to_string(), 10)];
    let err = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &empty_name).unwrap_err();
    assert!(matches!(err, RestoreLedgerError::EmptyArtifactName));

    // 8. Duplicate artifact name
    let dup_names = vec![
        ("part-0.bin".to_string(), 10),
        ("part-0.bin".to_string(), 20),
    ];
    let err = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &dup_names).unwrap_err();
    assert!(matches!(err, RestoreLedgerError::DuplicateArtifact(ref s) if s == "part-0.bin"));
}

#[test]
fn each_illegal_transition() {
    let artifacts = sample_artifacts();
    let mut ledger = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &artifacts).unwrap();
    let snapshot = ledger.clone();

    // 1. Unknown artifact in mark_verified
    let err = ledger.mark_verified("unknown.bin").unwrap_err();
    assert!(matches!(err, RestoreLedgerError::UnknownArtifact(ref s) if s == "unknown.bin"));
    assert_eq!(ledger, snapshot);

    // 2. begin_apply when artifacts are still pending
    let err = ledger.begin_apply().unwrap_err();
    assert!(matches!(err, RestoreLedgerError::UnverifiedArtifacts(_)));
    assert_eq!(ledger, snapshot);

    // 3. mark_applied while in Staging
    let err = ledger.mark_applied("data/part-0.bin").unwrap_err();
    assert!(matches!(
        err,
        RestoreLedgerError::IllegalPhaseTransition { .. }
    ));
    assert_eq!(ledger, snapshot);

    // 4. commit while in Staging
    let err = ledger.commit().unwrap_err();
    assert!(matches!(
        err,
        RestoreLedgerError::IllegalPhaseTransition { .. }
    ));
    assert_eq!(ledger, snapshot);

    // Transition to Applying
    ledger.mark_verified("data/part-0.bin").unwrap();
    ledger.mark_verified("data/part-1.bin").unwrap();
    ledger.mark_verified("data/part-2.bin").unwrap();
    ledger.begin_apply().unwrap();
    let snapshot_applying = ledger.clone();

    // 5. Unknown artifact in mark_applied
    let err = ledger.mark_applied("unknown.bin").unwrap_err();
    assert!(matches!(err, RestoreLedgerError::UnknownArtifact(ref s) if s == "unknown.bin"));
    assert_eq!(ledger, snapshot_applying);

    // 6. commit while artifacts are not yet applied
    let err = ledger.commit().unwrap_err();
    assert!(matches!(err, RestoreLedgerError::UnappliedArtifacts(_)));
    assert_eq!(ledger, snapshot_applying);

    // Complete apply and commit
    ledger.mark_applied("data/part-0.bin").unwrap();
    ledger.mark_applied("data/part-1.bin").unwrap();
    ledger.mark_applied("data/part-2.bin").unwrap();
    ledger.commit().unwrap();
    let snapshot_committed = ledger.clone();

    // 7. begin_apply while in Committed
    let err = ledger.begin_apply().unwrap_err();
    assert!(matches!(
        err,
        RestoreLedgerError::IllegalPhaseTransition { .. }
    ));
    assert_eq!(ledger, snapshot_committed);

    // 8. mark_applied while in Committed
    let err = ledger.mark_applied("data/part-0.bin").unwrap_err();
    assert!(matches!(
        err,
        RestoreLedgerError::IllegalPhaseTransition { .. }
    ));
    assert_eq!(ledger, snapshot_committed);
}

#[test]
fn idempotent_repeats() {
    let artifacts = sample_artifacts();
    let mut ledger = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &artifacts).unwrap();

    // In Staging: verify part-0
    ledger.mark_verified("data/part-0.bin").unwrap();
    assert_eq!(
        ledger.artifact_stage("data/part-0.bin"),
        Some(ArtifactStage::Verified)
    );
    // Repeating mark_verified on Verified is Ok
    ledger.mark_verified("data/part-0.bin").unwrap();
    assert_eq!(
        ledger.artifact_stage("data/part-0.bin"),
        Some(ArtifactStage::Verified)
    );

    ledger.mark_verified("data/part-1.bin").unwrap();
    ledger.mark_verified("data/part-2.bin").unwrap();
    ledger.begin_apply().unwrap();

    // Repeating begin_apply while Applying is Ok
    ledger.begin_apply().unwrap();
    assert_eq!(ledger.phase(), RestorePhase::Applying);

    // In Applying: apply part-0
    ledger.mark_applied("data/part-0.bin").unwrap();
    assert_eq!(
        ledger.artifact_stage("data/part-0.bin"),
        Some(ArtifactStage::Applied)
    );
    // Repeating mark_applied on Applied is Ok
    ledger.mark_applied("data/part-0.bin").unwrap();
    assert_eq!(
        ledger.artifact_stage("data/part-0.bin"),
        Some(ArtifactStage::Applied)
    );
    // Repeating mark_verified on Applied is Ok
    ledger.mark_verified("data/part-0.bin").unwrap();
    assert_eq!(
        ledger.artifact_stage("data/part-0.bin"),
        Some(ArtifactStage::Applied)
    );

    ledger.mark_applied("data/part-1.bin").unwrap();
    ledger.mark_applied("data/part-2.bin").unwrap();
    ledger.commit().unwrap();

    // Repeating commit while Committed is Ok
    ledger.commit().unwrap();
    assert_eq!(ledger.phase(), RestorePhase::Committed);
}

#[test]
fn conflicting_set_different_id_and_replaced_manifest() {
    let artifacts = sample_artifacts();
    let ledger = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &artifacts).unwrap();
    let snapshot = ledger.clone();

    // 1. Different set_id
    let err = ledger.resume("different-set-id", VALID_DIGEST).unwrap_err();
    assert_eq!(
        err,
        RestoreLedgerError::ConflictingSet {
            existing: VALID_SET_ID.to_string(),
            attempted: "different-set-id".to_string(),
        }
    );
    assert_eq!(ledger, snapshot);

    // 2. Same set_id with replaced manifest digest
    let err = ledger.resume(VALID_SET_ID, ALT_DIGEST).unwrap_err();
    assert_eq!(
        err,
        RestoreLedgerError::ConflictingSet {
            existing: VALID_DIGEST.to_string(),
            attempted: ALT_DIGEST.to_string(),
        }
    );
    assert_eq!(ledger, snapshot);
}

#[test]
fn already_committed_replay_and_resume_or_begin() {
    let artifacts = sample_artifacts();
    let mut ledger = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &artifacts).unwrap();

    // resume in Staging -> Continue
    let point = ledger.resume(VALID_SET_ID, VALID_DIGEST).unwrap();
    assert_eq!(point, ResumePoint::Continue);

    // resume_or_begin with None -> Fresh
    let (fresh, fresh_point) =
        resume_or_begin(None, VALID_SET_ID, VALID_DIGEST, &artifacts).unwrap();
    assert_eq!(fresh_point, ResumePoint::Fresh);
    assert_eq!(fresh.phase(), RestorePhase::Staging);

    // resume_or_begin with Staging -> Continue
    let (cont, cont_point) =
        resume_or_begin(Some(ledger.clone()), VALID_SET_ID, VALID_DIGEST, &artifacts).unwrap();
    assert_eq!(cont_point, ResumePoint::Continue);
    assert_eq!(cont.phase(), RestorePhase::Staging);

    // Drive ledger to Committed
    for a in &artifacts {
        ledger.mark_verified(&a.0).unwrap();
    }
    ledger.begin_apply().unwrap();
    for a in &artifacts {
        ledger.mark_applied(&a.0).unwrap();
    }
    ledger.commit().unwrap();

    // resume on Committed -> AlreadyCommitted
    let committed_point = ledger.resume(VALID_SET_ID, VALID_DIGEST).unwrap();
    assert_eq!(committed_point, ResumePoint::AlreadyCommitted);

    // resume_or_begin on Committed -> AlreadyCommitted
    let (res_ledger, res_point) = RestoreLedger::resume_or_begin(
        Some(ledger.clone()),
        VALID_SET_ID,
        VALID_DIGEST,
        &artifacts,
    )
    .unwrap();
    assert_eq!(res_point, ResumePoint::AlreadyCommitted);
    assert_eq!(res_ledger.phase(), RestorePhase::Committed);
}

#[test]
fn json_round_trip() {
    let artifacts = sample_artifacts();
    let mut ledger = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &artifacts).unwrap();

    // Round trip in Staging
    let json_bytes = ledger.to_json();
    let decoded = RestoreLedger::from_json(&json_bytes).unwrap();
    assert_eq!(ledger, decoded);

    // Round trip in Applying
    ledger.mark_verified("data/part-0.bin").unwrap();
    ledger.mark_verified("data/part-1.bin").unwrap();
    ledger.mark_verified("data/part-2.bin").unwrap();
    ledger.begin_apply().unwrap();
    ledger.mark_applied("data/part-0.bin").unwrap();
    let json_bytes = ledger.to_json();
    let decoded = RestoreLedger::from_json(&json_bytes).unwrap();
    assert_eq!(ledger, decoded);
    assert_eq!(
        decoded.unapplied(),
        vec!["data/part-1.bin", "data/part-2.bin"]
    );

    // Round trip in Committed
    ledger.mark_applied("data/part-1.bin").unwrap();
    ledger.mark_applied("data/part-2.bin").unwrap();
    ledger.commit().unwrap();
    let json_bytes = ledger.to_json();
    let decoded = RestoreLedger::from_json(&json_bytes).unwrap();
    assert_eq!(ledger, decoded);
    assert_eq!(decoded.phase(), RestorePhase::Committed);
    assert!(decoded.unapplied().is_empty());
}

#[test]
fn rejected_json_documents() {
    // 1. Oversized JSON (> 1 MiB)
    let huge_bytes = vec![b' '; MAX_RESTORE_LEDGER_BYTES + 1];
    let err = RestoreLedger::from_json(&huge_bytes).unwrap_err();
    assert!(matches!(
        err,
        RestoreLedgerError::OversizedJson {
            size: 1048577,
            max: 1048576
        }
    ));

    // 2. Unknown field
    let doc_unknown = json!({
        "version": 1,
        "setId": VALID_SET_ID,
        "manifestDigest": VALID_DIGEST,
        "phase": "staging",
        "unexpected": "reject-me",
        "artifacts": [
            { "name": "a.bin", "sizeBytes": 10, "stage": "pending" }
        ]
    });
    let err = RestoreLedger::from_json(&serde_json::to_vec(&doc_unknown).unwrap()).unwrap_err();
    assert!(matches!(err, RestoreLedgerError::Json(_)));

    // Unknown field inside artifact
    let doc_unknown_art = json!({
        "version": 1,
        "setId": VALID_SET_ID,
        "manifestDigest": VALID_DIGEST,
        "phase": "staging",
        "artifacts": [
            { "name": "a.bin", "sizeBytes": 10, "stage": "pending", "extra": true }
        ]
    });
    let err = RestoreLedger::from_json(&serde_json::to_vec(&doc_unknown_art).unwrap()).unwrap_err();
    assert!(matches!(err, RestoreLedgerError::Json(_)));

    // 3. Version 2
    let doc_v2 = json!({
        "version": 2,
        "setId": VALID_SET_ID,
        "manifestDigest": VALID_DIGEST,
        "phase": "staging",
        "artifacts": [
            { "name": "a.bin", "sizeBytes": 10, "stage": "pending" }
        ]
    });
    let err = RestoreLedger::from_json(&serde_json::to_vec(&doc_v2).unwrap()).unwrap_err();
    assert!(matches!(err, RestoreLedgerError::UnsupportedVersion(2)));

    // 4. Inconsistent phase: Staging with Applied artifact
    let doc_staging_applied = json!({
        "version": 1,
        "setId": VALID_SET_ID,
        "manifestDigest": VALID_DIGEST,
        "phase": "staging",
        "artifacts": [
            { "name": "a.bin", "sizeBytes": 10, "stage": "applied" }
        ]
    });
    let err =
        RestoreLedger::from_json(&serde_json::to_vec(&doc_staging_applied).unwrap()).unwrap_err();
    assert!(matches!(
        err,
        RestoreLedgerError::InconsistentPhase {
            phase: RestorePhase::Staging,
            ..
        }
    ));

    // 5. Inconsistent phase: Applying with Pending artifact
    let doc_applying_pending = json!({
        "version": 1,
        "setId": VALID_SET_ID,
        "manifestDigest": VALID_DIGEST,
        "phase": "applying",
        "artifacts": [
            { "name": "a.bin", "sizeBytes": 10, "stage": "pending" }
        ]
    });
    let err =
        RestoreLedger::from_json(&serde_json::to_vec(&doc_applying_pending).unwrap()).unwrap_err();
    assert!(matches!(
        err,
        RestoreLedgerError::InconsistentPhase {
            phase: RestorePhase::Applying,
            ..
        }
    ));

    // 6. Inconsistent phase: Committed with non-Applied artifact
    let doc_committed_verified = json!({
        "version": 1,
        "setId": VALID_SET_ID,
        "manifestDigest": VALID_DIGEST,
        "phase": "committed",
        "artifacts": [
            { "name": "a.bin", "sizeBytes": 10, "stage": "verified" }
        ]
    });
    let err = RestoreLedger::from_json(&serde_json::to_vec(&doc_committed_verified).unwrap())
        .unwrap_err();
    assert!(matches!(
        err,
        RestoreLedgerError::InconsistentPhase {
            phase: RestorePhase::Committed,
            ..
        }
    ));
}

#[test]
fn crash_prefix_model_test() {
    let art_names = ["art-0.bin", "art-1.bin", "art-2.bin"];
    let artifacts: Vec<(String, u64)> = art_names.iter().map(|n| (n.to_string(), 100)).collect();

    // The full sequence of transitions on a 3-artifact ledger:
    // 0: Initial state (begin)
    // 1: mark_verified(art-0)
    // 2: mark_verified(art-1)
    // 3: mark_verified(art-2)
    // 4: begin_apply()
    // 5: mark_applied(art-0)
    // 6: mark_applied(art-1)
    // 7: mark_applied(art-2)
    // 8: commit()
    let total_steps = 8;

    for prefix_len in 0..=total_steps {
        let mut ledger = RestoreLedger::begin(VALID_SET_ID, VALID_DIGEST, &artifacts).unwrap();

        // Track how many times each artifact was marked applied and actually changed state
        let mut applied_change_count: HashMap<&str, usize> =
            art_names.iter().map(|&n| (n, 0)).collect();

        // Execute prefix of transitions
        for step in 1..=prefix_len {
            match step {
                1 => ledger.mark_verified("art-0.bin").unwrap(),
                2 => ledger.mark_verified("art-1.bin").unwrap(),
                3 => ledger.mark_verified("art-2.bin").unwrap(),
                4 => ledger.begin_apply().unwrap(),
                5 => {
                    let art = "art-0.bin";
                    if ledger.unapplied().contains(&art) {
                        *applied_change_count.get_mut(art).unwrap() += 1;
                    }
                    ledger.mark_applied(art).unwrap();
                }
                6 => {
                    let art = "art-1.bin";
                    if ledger.unapplied().contains(&art) {
                        *applied_change_count.get_mut(art).unwrap() += 1;
                    }
                    ledger.mark_applied(art).unwrap();
                }
                7 => {
                    let art = "art-2.bin";
                    if ledger.unapplied().contains(&art) {
                        *applied_change_count.get_mut(art).unwrap() += 1;
                    }
                    ledger.mark_applied(art).unwrap();
                }
                8 => ledger.commit().unwrap(),
                _ => unreachable!(),
            }
        }

        // Crash and persist
        let serialized = ledger.to_json();

        // Reload from persisted JSON
        let mut reloaded = RestoreLedger::from_json(&serialized).expect("reload must succeed");

        // Resume with the same ids
        let resume_point = reloaded
            .resume(VALID_SET_ID, VALID_DIGEST)
            .expect("resume with same ids must succeed");

        if prefix_len == total_steps {
            assert_eq!(resume_point, ResumePoint::AlreadyCommitted);
        } else {
            assert_eq!(resume_point, ResumePoint::Continue);
        }

        // Replay the whole sequence on the reloaded ledger (idempotent repeats allowed)
        if resume_point != ResumePoint::AlreadyCommitted {
            // Replay verify on all artifacts (idempotent if already verified or applied)
            for &art in &art_names {
                reloaded.mark_verified(art).unwrap();
            }

            // Replay begin_apply (idempotent if already in Applying)
            reloaded.begin_apply().unwrap();

            // Replay apply on all artifacts (idempotent if already applied)
            for &art in &art_names {
                if reloaded.unapplied().contains(&art) {
                    *applied_change_count.get_mut(art).unwrap() += 1;
                }
                reloaded.mark_applied(art).unwrap();
            }

            // Replay commit
            reloaded.commit().unwrap();
        }

        // Ledger must now be in Committed phase
        assert_eq!(reloaded.phase(), RestorePhase::Committed);
        assert!(reloaded.unapplied().is_empty());
        assert!(reloaded.pending().is_empty());

        // Prove that every artifact was applied EXACTLY ONCE
        for &art in &art_names {
            assert_eq!(
                applied_change_count[art], 1,
                "artifact {art} was not applied exactly once for prefix length {prefix_len}"
            );
        }
    }
}
