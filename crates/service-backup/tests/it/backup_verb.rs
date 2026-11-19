//! The shared `backup` verb: one backup run from parameters, printed as the
//! pretty JSON of `BackupRunResult`, with one of three `next:` marker policies.

use service_backup::{
    render_backup_output, run_backup_verb, run_backup_verb_payload, BackupRunResult,
    BackupVerbParams, NextMarker,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn file_dest(root: &std::path::Path) -> String {
    format!("file://{}", root.display())
}

fn sample() -> BackupRunResult {
    serde_json::from_value(serde_json::json!({
        "object": {"sink": "local:/x", "key": "backup-1.json", "bytes": 3, "unixSeconds": 1},
        "pruned": 0
    }))
    .unwrap()
}

#[tokio::test]
async fn live_run_prints_the_result_json_and_writes_the_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"snapshot-bytes".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    let dest = file_dest(&root);

    let text = run_backup_verb(&BackupVerbParams {
        url: &server.uri(),
        token: None,
        dest: &dest,
        retention_secs: None,
        marker: NextMarker::None,
    })
    .await
    .unwrap();

    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(v["object"]["sink"].as_str().unwrap().starts_with("local:"));
    let key = v["object"]["key"].as_str().unwrap();
    assert!(key.starts_with("backup-") && key.ends_with(".json"));
    assert_eq!(v["object"]["bytes"], 14);
    assert!(v["object"]["unixSeconds"].as_u64().unwrap() > 0);
    assert_eq!(v["pruned"], 0);
    assert_eq!(std::fs::read(root.join(key)).unwrap(), b"snapshot-bytes");
    assert!(!text.contains("next"));
}

#[tokio::test]
async fn a_failed_fetch_writes_no_backup_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    let dest = file_dest(&root);

    let result = run_backup_verb(&BackupVerbParams {
        url: &server.uri(),
        token: None,
        dest: &dest,
        retention_secs: None,
        marker: NextMarker::None,
    })
    .await;

    assert!(result.is_err());
    assert!(!root.exists());
}

#[tokio::test]
async fn retention_removes_old_objects_and_reports_pruned() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    std::fs::create_dir_all(&root).unwrap();
    let old = root.join("backup-1.json");
    std::fs::write(&old, b"old").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH)
        .unwrap();
    let dest = file_dest(&root);

    let text = run_backup_verb_payload(b"new", &dest, Some(3600), NextMarker::None)
        .await
        .unwrap();

    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["pruned"], 1);
    assert!(!old.exists());
    assert!(root.join(v["object"]["key"].as_str().unwrap()).exists());
}

#[tokio::test]
async fn no_retention_prunes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    std::fs::create_dir_all(&root).unwrap();
    let old = root.join("backup-1.json");
    std::fs::write(&old, b"old").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH)
        .unwrap();
    let dest = file_dest(&root);

    let text = run_backup_verb_payload(b"new", &dest, None, NextMarker::None)
        .await
        .unwrap();

    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["pruned"], 0);
    assert!(old.exists());
}

#[tokio::test]
async fn the_token_is_sent_as_a_bearer_credential() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .and(header("authorization", "Bearer t-1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"x".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    let dest = file_dest(&root);

    let result = run_backup_verb(&BackupVerbParams {
        url: &server.uri(),
        token: Some("t-1"),
        dest: &dest,
        retention_secs: None,
        marker: NextMarker::None,
    })
    .await;

    assert!(result.is_ok());
}

#[test]
fn marker_none_is_one_json_document() {
    let text = render_backup_output(&sample(), NextMarker::None).unwrap();

    let _: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(!text.lines().any(|line| line.starts_with("next:")));
}

#[test]
fn marker_done_line_adds_a_separate_next_line() {
    let none = render_backup_output(&sample(), NextMarker::None).unwrap();
    let done = render_backup_output(&sample(), NextMarker::DoneLine).unwrap();

    assert!(done.ends_with("\nnext: done"));
    let body = &done[..done.len() - "\nnext: done".len()];
    let body_value: serde_json::Value = serde_json::from_str(body).unwrap();
    let none_value: serde_json::Value = serde_json::from_str(&none).unwrap();
    assert_eq!(body_value, none_value);
}

#[test]
fn marker_done_key_adds_next_inside_the_json_object() {
    let text = render_backup_output(&sample(), NextMarker::DoneKey).unwrap();

    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["next"], "done");
    assert_eq!(v["pruned"], 0);
    assert_eq!(v["object"]["key"], "backup-1.json");
    assert!(!text.contains("\nnext:"));
}

#[tokio::test]
async fn a_bad_destination_fails_before_any_request() {
    let server = MockServer::start().await;

    let result = run_backup_verb(&BackupVerbParams {
        url: &server.uri(),
        token: None,
        dest: "ftp://x",
        retention_secs: None,
        marker: NextMarker::None,
    })
    .await;

    assert!(result.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
}
