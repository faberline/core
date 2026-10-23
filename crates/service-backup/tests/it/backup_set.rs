use chrono::{DateTime, Utc};
use serde_json::json;
use service_backup::{
    ArtifactVerifier, BackupArtifact, BackupSetError, BackupSetV1, MAX_MANIFEST_BYTES,
};
use sha2::{Digest, Sha256};

fn fixed_timestamp() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-24T00:00:00Z")
        .expect("valid RFC 3339 timestamp")
        .with_timezone(&Utc)
}

fn sample_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

#[test]
fn valid_two_artifact_round_trip() {
    let payload1 = b"snapshot-data-part-1";
    let payload2 = b"snapshot-data-part-2";

    let set = BackupSetV1 {
        version: 1,
        set_id: "set-001".to_string(),
        created_at: fixed_timestamp(),
        producer: "service-backup".to_string(),
        artifacts: vec![
            BackupArtifact {
                name: "data/part-2.bin".to_string(),
                size_bytes: payload2.len() as u64,
                digest: sample_digest(payload2),
                content_type: "application/octet-stream".to_string(),
            },
            BackupArtifact {
                name: "data/part-1.bin".to_string(),
                size_bytes: payload1.len() as u64,
                digest: sample_digest(payload1),
                content_type: "application/octet-stream".to_string(),
            },
        ],
        service_metadata: Some(json!({
            "cluster": "prod-us-east-1",
            "shardId": 42
        })),
    };

    let canonical_bytes = set
        .to_canonical_json()
        .expect("canonical json serialization");
    let round_tripped = BackupSetV1::from_json(&canonical_bytes).expect("deserialization");

    // Artifacts in round_tripped should be sorted by name.
    let mut expected_sorted_set = set.clone();
    expected_sorted_set
        .artifacts
        .sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(round_tripped, expected_sorted_set);
}

#[test]
fn canonical_bytes_equal_under_reordered_artifacts_and_metadata_keys() {
    let p1 = b"artifact-one";
    let p2 = b"artifact-two";

    let set_a = BackupSetV1 {
        version: 1,
        set_id: "set-abc".to_string(),
        created_at: fixed_timestamp(),
        producer: "axiom-backup".to_string(),
        artifacts: vec![
            BackupArtifact {
                name: "b-part.bin".to_string(),
                size_bytes: p2.len() as u64,
                digest: sample_digest(p2),
                content_type: "application/octet-stream".to_string(),
            },
            BackupArtifact {
                name: "a-part.bin".to_string(),
                size_bytes: p1.len() as u64,
                digest: sample_digest(p1),
                content_type: "application/octet-stream".to_string(),
            },
        ],
        service_metadata: Some(json!({
            "zebra": 99,
            "nested": {
                "beta": "two",
                "alpha": "one"
            },
            "apple": true
        })),
    };

    let set_b = BackupSetV1 {
        version: 1,
        set_id: "set-abc".to_string(),
        created_at: fixed_timestamp(),
        producer: "axiom-backup".to_string(),
        artifacts: vec![
            BackupArtifact {
                name: "a-part.bin".to_string(),
                size_bytes: p1.len() as u64,
                digest: sample_digest(p1),
                content_type: "application/octet-stream".to_string(),
            },
            BackupArtifact {
                name: "b-part.bin".to_string(),
                size_bytes: p2.len() as u64,
                digest: sample_digest(p2),
                content_type: "application/octet-stream".to_string(),
            },
        ],
        service_metadata: Some(json!({
            "apple": true,
            "nested": {
                "alpha": "one",
                "beta": "two"
            },
            "zebra": 99
        })),
    };

    let bytes_a = set_a.to_canonical_json().expect("to_canonical_json set_a");
    let bytes_b = set_b.to_canonical_json().expect("to_canonical_json set_b");

    assert_eq!(bytes_a, bytes_b);
}

#[test]
fn invalid_name() {
    let invalid_names = ["../x", "a//b", "/a", "a space", "a/"];

    for invalid in invalid_names {
        let set = BackupSetV1 {
            version: 1,
            set_id: "set-001".to_string(),
            created_at: fixed_timestamp(),
            producer: "service-backup".to_string(),
            artifacts: vec![BackupArtifact {
                name: invalid.to_string(),
                size_bytes: 10,
                digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
                content_type: "application/octet-stream".to_string(),
            }],
            service_metadata: None,
        };

        match set.validate() {
            Err(BackupSetError::InvalidArtifactName(name)) => assert_eq!(name, invalid),
            other => panic!("expected InvalidArtifactName for `{invalid}`, got: {other:?}"),
        }
    }
}

#[test]
fn duplicate_name() {
    let set = BackupSetV1 {
        version: 1,
        set_id: "set-001".to_string(),
        created_at: fixed_timestamp(),
        producer: "service-backup".to_string(),
        artifacts: vec![
            BackupArtifact {
                name: "data/part.bin".to_string(),
                size_bytes: 10,
                digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
                content_type: "application/octet-stream".to_string(),
            },
            BackupArtifact {
                name: "data/part.bin".to_string(),
                size_bytes: 20,
                digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
                content_type: "application/octet-stream".to_string(),
            },
        ],
        service_metadata: None,
    };

    match set.validate() {
        Err(BackupSetError::DuplicateArtifactName(name)) => {
            assert_eq!(name, "data/part.bin");
        }
        other => panic!("expected DuplicateArtifactName, got: {other:?}"),
    }
}

#[test]
fn version_two() {
    let set = BackupSetV1 {
        version: 2,
        set_id: "set-001".to_string(),
        created_at: fixed_timestamp(),
        producer: "service-backup".to_string(),
        artifacts: vec![BackupArtifact {
            name: "data/part.bin".to_string(),
            size_bytes: 10,
            digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            content_type: "application/octet-stream".to_string(),
        }],
        service_metadata: None,
    };

    match set.validate() {
        Err(BackupSetError::UnsupportedVersion(v)) => assert_eq!(v, 2),
        other => panic!("expected UnsupportedVersion(2), got: {other:?}"),
    }

    let json_str = r#"{
        "version": 2,
        "setId": "set-001",
        "createdAt": "2026-10-24T00:00:00Z",
        "producer": "service-backup",
        "artifacts": [{
            "name": "data/part.bin",
            "sizeBytes": 10,
            "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "contentType": "application/octet-stream"
        }]
    }"#;

    match BackupSetV1::from_json(json_str.as_bytes()) {
        Err(BackupSetError::UnsupportedVersion(v)) => assert_eq!(v, 2),
        other => panic!("expected UnsupportedVersion(2) from JSON, got: {other:?}"),
    }
}

#[test]
fn unknown_field() {
    let json_str = r#"{
        "version": 1,
        "setId": "set-001",
        "createdAt": "2026-10-24T00:00:00Z",
        "producer": "service-backup",
        "artifacts": [{
            "name": "data/part.bin",
            "sizeBytes": 10,
            "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "contentType": "application/octet-stream"
        }],
        "unknownProperty": "not_allowed"
    }"#;

    match BackupSetV1::from_json(json_str.as_bytes()) {
        Err(BackupSetError::Json(err)) => {
            assert!(
                err.contains("unknown field"),
                "expected unknown field error, got: {err}"
            );
        }
        other => panic!("expected Json error for unknown field, got: {other:?}"),
    }
}

#[test]
fn oversized_manifest() {
    let mut oversized = vec![b' '; MAX_MANIFEST_BYTES + 1];
    oversized[0] = b'{';
    let len = oversized.len();
    oversized[len - 1] = b'}';

    match BackupSetV1::from_json(&oversized) {
        Err(BackupSetError::ManifestTooLarge { size, max }) => {
            assert_eq!(size, MAX_MANIFEST_BYTES + 1);
            assert_eq!(max, MAX_MANIFEST_BYTES);
        }
        other => panic!("expected ManifestTooLarge, got: {other:?}"),
    }
}

#[test]
fn non_object_service_metadata() {
    let json_str = r#"{
        "version": 1,
        "setId": "set-001",
        "createdAt": "2026-10-24T00:00:00Z",
        "producer": "service-backup",
        "artifacts": [{
            "name": "data/part.bin",
            "sizeBytes": 10,
            "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "contentType": "application/octet-stream"
        }],
        "serviceMetadata": "not-an-object"
    }"#;

    match BackupSetV1::from_json(json_str.as_bytes()) {
        Err(BackupSetError::InvalidServiceMetadata) => {}
        other => panic!("expected InvalidServiceMetadata, got: {other:?}"),
    }

    let set = BackupSetV1 {
        version: 1,
        set_id: "set-001".to_string(),
        created_at: fixed_timestamp(),
        producer: "service-backup".to_string(),
        artifacts: vec![BackupArtifact {
            name: "data/part.bin".to_string(),
            size_bytes: 10,
            digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            content_type: "application/octet-stream".to_string(),
        }],
        service_metadata: Some(json!([1, 2, 3])),
    };

    match set.validate() {
        Err(BackupSetError::InvalidServiceMetadata) => {}
        other => panic!("expected InvalidServiceMetadata, got: {other:?}"),
    }
}

#[test]
fn verifier_chunked_pass() {
    let payload = b"streaming chunks of data that verify cleanly";
    let artifact = BackupArtifact {
        name: "data/stream.bin".to_string(),
        size_bytes: payload.len() as u64,
        digest: sample_digest(payload),
        content_type: "application/octet-stream".to_string(),
    };

    let mut verifier = ArtifactVerifier::new(&artifact);
    let chunk1 = &payload[..10];
    let chunk2 = &payload[10..25];
    let chunk3 = &payload[25..];

    verifier.update(chunk1).expect("update chunk1");
    verifier.update(chunk2).expect("update chunk2");
    verifier.update(chunk3).expect("update chunk3");
    verifier.finish().expect("finish successfully");
}

#[test]
fn verifier_failures() {
    let payload = b"hello world";
    let artifact = BackupArtifact {
        name: "data/file.bin".to_string(),
        size_bytes: payload.len() as u64,
        digest: sample_digest(payload),
        content_type: "application/octet-stream".to_string(),
    };

    // 1. Too many bytes
    let mut too_many = ArtifactVerifier::new(&artifact);
    too_many.update(payload).expect("exact bytes update");
    match too_many.update(b"extra") {
        Err(BackupSetError::BytesExceeded { expected, actual }) => {
            assert_eq!(expected, payload.len() as u64);
            assert_eq!(actual, (payload.len() + 5) as u64);
        }
        other => panic!("expected BytesExceeded, got: {other:?}"),
    }

    // 2. Too few bytes
    let mut too_few = ArtifactVerifier::new(&artifact);
    too_few.update(&payload[..5]).expect("partial update");
    match too_few.finish() {
        Err(BackupSetError::ShortSize { expected, actual }) => {
            assert_eq!(expected, payload.len() as u64);
            assert_eq!(actual, 5);
        }
        other => panic!("expected ShortSize, got: {other:?}"),
    }

    // 3. Wrong digest
    let wrong_payload = b"hello earth";
    let mut wrong_digest = ArtifactVerifier::new(&artifact);
    wrong_digest
        .update(wrong_payload)
        .expect("same size update");
    match wrong_digest.finish() {
        Err(BackupSetError::DigestMismatch { expected, actual }) => {
            assert_eq!(expected, artifact.digest);
            assert_eq!(actual, sample_digest(wrong_payload));
        }
        other => panic!("expected DigestMismatch, got: {other:?}"),
    }
}
