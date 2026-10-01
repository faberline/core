//! Golden pins for the admin snapshot request error text, taken before the DDD
//! P2 changes: a projected bearer that cannot be read or validated fails with
//! the same redacted message, before any request is sent.
#![cfg(feature = "http-client")]

use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Serialize;
use service_backup::{AdminSnapshotRequest, AdminSnapshotTransport};

#[derive(Serialize)]
struct Claims<'a> {
    aud: [&'a str; 1],
    exp: u64,
}

const CREDENTIAL_FAILED: &str = "admin snapshot backup credential could not be read or validated";

async fn fetch_error(request: &AdminSnapshotRequest) -> (String, String, bool) {
    let error = AdminSnapshotTransport::new()
        .unwrap()
        .fetch("http://127.0.0.1:9", request)
        .await
        .unwrap_err();
    let diagnostic = error.diagnostic().is_some();
    let error = anyhow::Error::from(error);
    (error.to_string(), format!("{error:#}"), diagnostic)
}

#[tokio::test]
async fn missing_projected_token_error_is_pinned() {
    let dir = tempfile::tempdir().unwrap();
    let request = AdminSnapshotRequest::new()
        .with_projected_bearer(dir.path().join("missing-token"), "sift.axiom.dev");
    assert_eq!(
        fetch_error(&request).await,
        (CREDENTIAL_FAILED.into(), CREDENTIAL_FAILED.into(), false)
    );
}

#[tokio::test]
async fn wrong_audience_projected_token_error_is_pinned_and_redacted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("token");
    let token = encode(
        &Header::new(jsonwebtoken::Algorithm::HS256),
        &Claims {
            aud: ["someone-else"],
            exp: 4_102_444_800,
        },
        &EncodingKey::from_secret(b"key"),
    )
    .unwrap();
    std::fs::write(&path, &token).unwrap();
    let request = AdminSnapshotRequest::new().with_projected_bearer(&path, "sift.axiom.dev");
    let (display, chain, diagnostic) = fetch_error(&request).await;
    assert_eq!(
        (display.as_str(), chain.as_str(), diagnostic),
        (CREDENTIAL_FAILED, CREDENTIAL_FAILED, false)
    );
    assert!(!chain.contains(&token));
}
