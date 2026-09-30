//! Golden pins for the backup CRD wire shapes, the rendered LLM topic and the
//! destination/policy error text, taken before the DDD P2 changes. Each JSON
//! literal is asserted both ways: encode gives the literal and decoding the
//! literal gives the value back.

use service_backup::{
    llm, BackupDestination, BackupPolicy, RetentionPolicy, ScheduledBackupPolicy,
};

fn round_trip<T>(value: &T, json: &str)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(serde_json::to_string(value).unwrap(), json);
    assert_eq!(&serde_json::from_str::<T>(json).unwrap(), value);
}

fn s3_full() -> BackupDestination {
    BackupDestination::S3 {
        bucket: "b".into(),
        prefix: "p".into(),
        region: Some("us-east-1".into()),
        endpoint: Some("http://minio:9000".into()),
        credentials_secret: Some("s3-creds".into()),
    }
}

#[test]
fn backup_destination_local_json_is_pinned() {
    round_trip(
        &BackupDestination::Local {
            path: "/var/backups".into(),
            prefix: None,
        },
        r#"{"type":"local","path":"/var/backups"}"#,
    );
    round_trip(
        &BackupDestination::Local {
            path: "/var/backups".into(),
            prefix: Some("nightly".into()),
        },
        r#"{"type":"local","path":"/var/backups","prefix":"nightly"}"#,
    );
}

#[test]
fn backup_destination_s3_json_is_pinned() {
    round_trip(
        &BackupDestination::S3 {
            bucket: "b".into(),
            prefix: String::new(),
            region: None,
            endpoint: None,
            credentials_secret: None,
        },
        r#"{"type":"s3","bucket":"b","prefix":""}"#,
    );
    round_trip(
        &s3_full(),
        r#"{"type":"s3","bucket":"b","prefix":"p","region":"us-east-1","endpoint":"http://minio:9000","credentials_secret":"s3-creds"}"#,
    );
    assert_eq!(
        serde_json::from_str::<BackupDestination>(r#"{"type":"s3","bucket":"b"}"#).unwrap(),
        BackupDestination::S3 {
            bucket: "b".into(),
            prefix: String::new(),
            region: None,
            endpoint: None,
            credentials_secret: None,
        }
    );
}

#[test]
fn backup_destination_gcs_json_is_pinned() {
    round_trip(
        &BackupDestination::Gcs {
            bucket: "g".into(),
            prefix: String::new(),
            credentials_secret: None,
        },
        r#"{"type":"gcs","bucket":"g","prefix":""}"#,
    );
    round_trip(
        &BackupDestination::Gcs {
            bucket: "g".into(),
            prefix: "p".into(),
            credentials_secret: Some("gcs-creds".into()),
        },
        r#"{"type":"gcs","bucket":"g","prefix":"p","credentials_secret":"gcs-creds"}"#,
    );
    assert_eq!(
        serde_json::from_str::<BackupDestination>(r#"{"type":"gcs","bucket":"g"}"#).unwrap(),
        BackupDestination::Gcs {
            bucket: "g".into(),
            prefix: String::new(),
            credentials_secret: None,
        }
    );
}

#[test]
fn scheduled_backup_policy_json_is_pinned() {
    round_trip(
        &ScheduledBackupPolicy {
            schedule: "0 * * * *".into(),
            destination: "s3://bucket/prefix".into(),
            retention_secs: Some(3600),
        },
        r#"{"schedule":"0 * * * *","destination":"s3://bucket/prefix","retentionSecs":3600}"#,
    );
    round_trip(
        &ScheduledBackupPolicy {
            schedule: "0 * * * *".into(),
            destination: "gs://bucket".into(),
            retention_secs: None,
        },
        r#"{"schedule":"0 * * * *","destination":"gs://bucket"}"#,
    );
}

#[test]
fn backup_policy_and_retention_json_are_pinned() {
    round_trip(
        &BackupPolicy {
            schedule: "0 * * * *".into(),
            destination: s3_full(),
            retention: RetentionPolicy::max_age_seconds(3600),
        },
        r#"{"schedule":"0 * * * *","destination":{"type":"s3","bucket":"b","prefix":"p","region":"us-east-1","endpoint":"http://minio:9000","credentials_secret":"s3-creds"},"retention":{"maxAgeSeconds":3600}}"#,
    );
    assert_eq!(
        serde_json::from_str::<BackupPolicy>(
            r#"{"schedule":"@daily","destination":{"type":"local","path":"/x"}}"#
        )
        .unwrap(),
        BackupPolicy {
            schedule: "@daily".into(),
            destination: BackupDestination::Local {
                path: "/x".into(),
                prefix: None,
            },
            retention: RetentionPolicy::default(),
        }
    );
    round_trip(&RetentionPolicy::default(), "{}");
    round_trip(
        &RetentionPolicy::max_age_seconds(60),
        r#"{"maxAgeSeconds":60}"#,
    );
}

fn chain(err: impl Into<anyhow::Error>) -> (String, String) {
    let err = err.into();
    (err.to_string(), format!("{err:#}"))
}

fn flat(message: &str) -> (String, String) {
    (message.to_string(), message.to_string())
}

#[test]
fn destination_uri_errors_are_pinned() {
    for (uri, message) in [
        ("", "backup destination URI is empty"),
        ("  ", "backup destination URI is empty"),
        ("file://", "file backup URI has no path"),
        ("s3://", "s3 backup URI has no bucket"),
        ("s3:///p", "s3 backup URI has no bucket"),
        ("gs://", "gs backup URI has no bucket"),
        ("gs:///p", "gs backup URI has no bucket"),
        (
            "ftp://nope",
            "unsupported backup destination URI `ftp://nope`; use file://, s3://, gs://",
        ),
    ] {
        let err = BackupDestination::from_uri(uri).unwrap_err();
        assert_eq!(chain(err), flat(message), "uri {uri:?}");
    }
}

#[test]
fn scheduled_policy_conversion_errors_are_pinned() {
    for (schedule, destination, message) in [
        ("  ", "s3://b/p", "backup schedule must not be empty"),
        (
            "0 * * * *",
            "ftp://x",
            "unsupported backup destination URI `ftp://x`; use file://, s3://, gs://",
        ),
        ("0 * * * *", " ", "backup destination URI is empty"),
    ] {
        let policy = ScheduledBackupPolicy {
            schedule: schedule.into(),
            destination: destination.into(),
            retention_secs: None,
        };
        let err = policy.to_runtime_policy().unwrap_err();
        assert_eq!(chain(err), flat(message), "to_runtime_policy {policy:?}");
        let err = BackupPolicy::try_from(&policy).unwrap_err();
        assert_eq!(chain(err), flat(message), "try_from {policy:?}");
    }
}

const TOPIC_HEAD: &str = r#"# service-backup shared topic

## Ownership boundary
The service owns snapshot consistency, snapshot bytes, restore semantics, and
the admin or CLI endpoint that produces the snapshot. `service-backup` owns the
transport contract around those bytes: `BackupDestination`, `RetentionPolicy`,
`BackupSink`, `LocalFsSink`, optional S3 support, `run_backup_once`,
`fetch_backup_object`, and (with `http-client`) the authenticated standard
`GET /admin/backup` fetch/upload path.

Operator code or a CronJob should schedule and transport backups; it should not
serialize service state itself.

## Destination contract

Supported destination URI schemes in this build:

- `file://` — local filesystem path — dev/tests and PVC-backed local runs (sink linked into this build)
"#;

const S3_LINKED: &str =
    "- `s3://` — Amazon S3-compatible object store (sink linked into this build)\n";

const S3_UNLINKED: &str = "- `s3://` — Amazon S3-compatible object store (parses, but no sink linked — uploads fail loud until rebuilt with the adapter feature)\n";

const TOPIC_TAIL: &str = r#"- `gs://` — Google Cloud Storage — workload identity in production, `STORAGE_EMULATOR_HOST` locally (sink linked into this build)

Example: `s3://bucket/prefix`, `gs://bucket/prefix`, `file:///mnt/backups/service`.


## Restore and bootstrap
`fetch_backup_object` reads an exact object URI (any scheme from the
destination contract above) for restore or empty-PVC bootstrap. It is a cold
seed path, not live replica synchronization."#;

#[test]
fn rendered_sectioned_llm_topic_is_pinned() {
    let s3 = if cfg!(feature = "s3") {
        S3_LINKED
    } else {
        S3_UNLINKED
    };
    let rendered = cli_std::llm::render_sectioned(
        "x",
        "0",
        llm::SECTIONED_TOPICS,
        "service-backup",
        cli_std::llm::Format::Md,
    )
    .unwrap();
    assert_eq!(rendered, format!("{TOPIC_HEAD}{s3}{TOPIC_TAIL}"));
}

#[test]
fn static_llm_topic_is_pinned() {
    let topic = llm::topic();
    assert_eq!(topic.id(), "service-backup");
    assert_eq!(
        topic.summary(),
        "Shared backup destination, policy, sink, runner, and bootstrap-object contract."
    );
    assert!(topic
        .body()
        .starts_with("# service-backup shared topic\n\n## Ownership boundary\n"));
}
