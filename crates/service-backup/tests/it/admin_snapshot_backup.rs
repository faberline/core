//! `run_admin_snapshot_backup` fetches through the strict transport: exactly
//! `200 OK`, no redirects, and a redacted error that leaves no local backup
//! directory behind.

use service_backup::{run_admin_snapshot_backup, BackupDestination, RetentionPolicy};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn local(dir: &std::path::Path) -> BackupDestination {
    BackupDestination::Local {
        path: dir.to_string_lossy().into_owned(),
        prefix: Some("svc".into()),
    }
}

#[tokio::test]
async fn writes_the_exact_200_bytes_to_the_destination() {
    let server = MockServer::start().await;
    let payload = b"bytes\x00with\xffopaque".to_vec();
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .and(header("authorization", "Bearer admin-token"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");

    let result = run_admin_snapshot_backup(
        &server.uri(),
        Some("admin-token"),
        &local(&root),
        &RetentionPolicy::default(),
    )
    .await
    .unwrap();

    assert_eq!(result.object.bytes, payload.len());
    assert_eq!(result.pruned, 0);
    assert_eq!(
        std::fs::read(root.join(&result.object.key)).unwrap(),
        payload
    );
}

#[tokio::test]
async fn a_non_200_status_is_redacted_and_opens_no_sink() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .respond_with(ResponseTemplate::new(503).set_body_string("not ready: secret-detail"))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");

    let error = run_admin_snapshot_backup(
        &server.uri(),
        None,
        &local(&root),
        &RetentionPolicy::default(),
    )
    .await
    .unwrap_err();

    let expected = "admin snapshot Backup returned unexpected status 503 Service Unavailable";
    assert_eq!(error.to_string(), expected);
    assert_eq!(format!("{error:#}"), expected);
    assert!(
        !root.exists(),
        "a failed fetch must not create the directory"
    );
}

#[tokio::test]
async fn a_redirect_is_not_followed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", "http://127.0.0.1:9/admin/backup"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();

    let error = run_admin_snapshot_backup(
        &server.uri(),
        None,
        &local(dir.path()),
        &RetentionPolicy::default(),
    )
    .await
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "admin snapshot Backup returned unexpected status 302 Found"
    );
}

#[test]
fn the_backup_future_is_send() {
    fn assert_send<T: Send>(_: &T) {}
    let dest = BackupDestination::Local {
        path: "unused".into(),
        prefix: None,
    };
    let retention = RetentionPolicy::default();
    let future = run_admin_snapshot_backup("http://127.0.0.1:9", None, &dest, &retention);
    assert_send(&future);
}
