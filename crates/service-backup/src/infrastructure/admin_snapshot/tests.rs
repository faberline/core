use std::time::Duration;

use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use wiremock::matchers::{body_bytes, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fetches_exact_snapshot_bytes_with_bearer_auth() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .and(header("authorization", "Bearer registry-token"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"snapshot".to_vec()))
        .mount(&server)
        .await;

    let bytes = fetch_admin_snapshot(&server.uri(), Some("registry-token"))
        .await
        .expect("admin snapshot fetch succeeds");
    assert_eq!(bytes, b"snapshot");
}

#[tokio::test]
async fn keeps_non_success_status_and_body_in_the_diagnostic() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .respond_with(ResponseTemplate::new(503).set_body_string("not ready"))
        .mount(&server)
        .await;

    let error = fetch_admin_snapshot(&server.uri(), None)
        .await
        .expect_err("non-success status must fail");
    assert!(error.to_string().contains("503"));
    assert!(error.to_string().contains("not ready"));
}

#[tokio::test]
async fn strict_backup_accepts_only_200_and_has_no_bearer_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .respond_with(ResponseTemplate::new(201).set_body_bytes(b"secret-payload".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    let transport =
        AdminSnapshotTransport::for_tests(Duration::from_millis(200), Duration::from_millis(50));
    let error = transport
        .fetch_exact(&server.uri(), None)
        .await
        .expect_err("201 is not exact 200");
    assert_eq!(
        error,
        AdminSnapshotTransportError::UnexpectedStatus {
            operation: AdminSnapshotOperation::Backup,
            status: reqwest::StatusCode::CREATED,
        }
    );
    assert!(!error.to_string().contains("secret-payload"));
}

#[tokio::test]
async fn strict_backup_preserves_exact_200_bytes_without_bearer() {
    let server = MockServer::start().await;
    let payload = b"bytes\x00with\xffopaque".to_vec();
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
        .expect(1)
        .mount(&server)
        .await;
    let transport =
        AdminSnapshotTransport::for_tests(Duration::from_millis(200), Duration::from_millis(50));
    assert_eq!(
        transport.fetch_exact(&server.uri(), None).await.unwrap(),
        payload
    );
}

#[tokio::test]
async fn strict_restore_preserves_body_and_content_type_and_accepts_only_204() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/admin/restore"))
        .and(header("content-type", "application/json"))
        .and(header("authorization", "Bearer restore-token"))
        .and(body_bytes(b"{\"opaque\":true}"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let transport =
        AdminSnapshotTransport::for_tests(Duration::from_millis(200), Duration::from_millis(50));
    transport
        .restore_exact(&server.uri(), Some("restore-token"), b"{\"opaque\":true}")
        .await
        .expect("restore succeeds");
}

#[tokio::test]
async fn strict_restore_rejects_other_success_status_without_body_leak() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/admin/restore"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"canary-body".to_vec()))
        .mount(&server)
        .await;
    let transport =
        AdminSnapshotTransport::for_tests(Duration::from_millis(200), Duration::from_millis(50));
    let error = transport
        .restore_exact(&server.uri(), Some("canary-token"), b"canary-payload")
        .await
        .expect_err("200 is not exact 204");
    assert_eq!(
        error,
        AdminSnapshotTransportError::UnexpectedStatus {
            operation: AdminSnapshotOperation::Restore,
            status: reqwest::StatusCode::OK,
        }
    );
    let text = error.to_string();
    assert!(!text.contains("canary"));
}

#[tokio::test]
async fn strict_backup_does_not_follow_redirects() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", "http://127.0.0.1:1/admin/backup"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let transport =
        AdminSnapshotTransport::for_tests(Duration::from_millis(200), Duration::from_millis(50));
    let error = transport
        .fetch_exact(&server.uri(), Some("redirect-token"))
        .await
        .expect_err("redirect is not 200");
    assert!(
        matches!(error, AdminSnapshotTransportError::UnexpectedStatus { status, .. } if status == reqwest::StatusCode::FOUND)
    );
    assert!(!error.to_string().contains("redirect-token"));
}

#[tokio::test]
async fn strict_request_timeout_is_redacted() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/backup"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(150)))
        .mount(&server)
        .await;
    let transport =
        AdminSnapshotTransport::for_tests(Duration::from_millis(40), Duration::from_millis(20));
    let error = transport
        .fetch_exact(&server.uri(), Some("timeout-token"))
        .await
        .expect_err("request times out");
    assert_eq!(
        error,
        AdminSnapshotTransportError::RequestFailed {
            operation: AdminSnapshotOperation::Backup
        }
    );
    assert!(!error.to_string().contains("timeout-token"));
}

#[tokio::test]
async fn strict_backup_idle_timeout_maps_stalled_body_read() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("listener address");
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept test request");
        let mut request = [0_u8; 2048];
        let _ = socket.read(&mut request).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\na")
            .await
            .expect("write initial chunk");
        tokio::time::sleep(Duration::from_millis(120)).await;
        let _ = socket.write_all(b"b").await;
    });
    let transport =
        AdminSnapshotTransport::for_tests(Duration::from_millis(500), Duration::from_millis(40));
    let error = transport
        .fetch_exact(&format!("http://{address}"), None)
        .await
        .expect_err("stalled body must fail");
    assert_eq!(
        error,
        AdminSnapshotTransportError::ResponseReadFailed {
            operation: AdminSnapshotOperation::Backup
        }
    );
    task.await.expect("server task exits");
}

#[tokio::test]
async fn strict_backup_idle_timeout_resets_for_each_chunk() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("listener address");
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept test request");
        let mut request = [0_u8; 2048];
        let _ = socket.read(&mut request).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\na")
            .await
            .expect("write first chunk");
        tokio::time::sleep(Duration::from_millis(45)).await;
        socket.write_all(b"b").await.expect("write second chunk");
        tokio::time::sleep(Duration::from_millis(45)).await;
        socket.write_all(b"c").await.expect("write final chunk");
    });
    let transport =
        AdminSnapshotTransport::for_tests(Duration::from_millis(500), Duration::from_millis(80));
    assert_eq!(
        transport
            .fetch_exact(&format!("http://{address}"), None)
            .await
            .expect("chunks arrive within idle timeout"),
        b"abc"
    );
    task.await.expect("server task exits");
}

#[tokio::test]
async fn strict_transport_failure_uses_one_connection_without_retry() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("listener address");
    let connections = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&connections);
    let task = tokio::spawn(async move {
        let (mut socket, _) = tokio::time::timeout(Duration::from_millis(150), listener.accept())
            .await
            .expect("first connection arrives")
            .expect("first accept succeeds");
        seen.fetch_add(1, Ordering::SeqCst);
        let mut request = [0_u8; 2048];
        let _ = socket.read(&mut request).await;
        let _ = socket.shutdown().await;
        if let Ok(Ok((mut retry_socket, _))) =
            tokio::time::timeout(Duration::from_millis(150), listener.accept()).await
        {
            seen.fetch_add(1, Ordering::SeqCst);
            let _ = retry_socket.shutdown().await;
        }
    });
    let transport =
        AdminSnapshotTransport::for_tests(Duration::from_millis(300), Duration::from_millis(30));
    let error = transport
        .fetch_exact(&format!("http://{address}"), None)
        .await
        .expect_err("closed response must fail");
    assert!(matches!(
        error,
        AdminSnapshotTransportError::RequestFailed {
            operation: AdminSnapshotOperation::Backup
        } | AdminSnapshotTransportError::ResponseReadFailed {
            operation: AdminSnapshotOperation::Backup
        }
    ));
    task.await.expect("server task exits");
    assert_eq!(connections.load(Ordering::SeqCst), 1);
}
