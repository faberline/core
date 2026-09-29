use super::*;
use std::time::Instant;

#[test]
fn the_exchange_presents_a_ksa_assertion_and_nothing_else() {
    let source = WorkloadIdentityTokenSource::new(
        "/var/run/secrets/tokens/gcp-ksa/token",
        "//iam.googleapis.com/projects/1/locations/global/workloadIdentityPools/p/providers/v",
    );
    let form: std::collections::BTreeMap<&str, String> =
        source.exchange_form("assertion").into_iter().collect();
    assert_eq!(
        form["grant_type"],
        "urn:ietf:params:oauth:grant-type:token-exchange"
    );
    assert_eq!(
        form["subject_token_type"],
        "urn:ietf:params:oauth:token-type:jwt"
    );
    assert_eq!(form["subject_token"], "assertion");
    assert!(
        !form.contains_key("client_secret") && !form.contains_key("assertion_type"),
        "there is no long-lived credential in this exchange; a field for one is a field \
         somebody will eventually fill in"
    );
}

#[tokio::test]
async fn gke_metadata_token_source_fetches_token_with_required_header() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();

    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        if let Ok((mut stream, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);
            // Case-insensitive header check per RFC 7230
            let lower_req = req_str.to_lowercase();
            assert!(lower_req.contains("metadata-flavor: google"));
            let body = json!({
                "access_token": "secret-oauth-token-12345",
                "expires_in": 3600,
                "token_type": "Bearer"
            })
            .to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).await.unwrap();
        }
    });

    let source = GkeMetadataTokenSource::new().with_endpoint(format!(
        "http://127.0.0.1:{port}/computeMetadata/v1/instance/service-accounts/default/token"
    ));
    let token = source.token().await.unwrap();
    assert_eq!(token, "secret-oauth-token-12345");

    // Cache reuse: second call does not reach server (server closed after 1 request)
    let token2 = source.token().await.unwrap();
    assert_eq!(token2, "secret-oauth-token-12345");
}

#[test]
fn gke_metadata_token_source_default_endpoint_matches_documented_gke_metadata_url() {
    let source = GkeMetadataTokenSource::new();
    assert_eq!(
        source.endpoint(),
        "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token"
    );
    assert_eq!(source.endpoint(), GKE_METADATA_TOKEN_ENDPOINT);
}

#[tokio::test]
async fn gke_metadata_token_source_does_not_cache_short_lived_tokens() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();

    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        for token_val in ["token-short-1", "token-short-2"] {
            if let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let body = json!({
                    "access_token": token_val,
                    "expires_in": 200, // <= 300s margin -> not cached
                    "token_type": "Bearer"
                })
                .to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(resp.as_bytes()).await.unwrap();
            }
        }
    });

    let source =
        GkeMetadataTokenSource::new().with_endpoint(format!("http://127.0.0.1:{port}/token"));
    let token1 = source.token().await.unwrap();
    assert_eq!(token1, "token-short-1");

    // Second call fetches fresh token because expires_in <= 300
    let token2 = source.token().await.unwrap();
    assert_eq!(token2, "token-short-2");
}

#[tokio::test]
async fn gke_metadata_token_source_fetches_fresh_token_when_cache_is_stale() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();

    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        if let Ok((mut stream, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let body = json!({
                "access_token": "fresh-token-from-server",
                "expires_in": 3600,
                "token_type": "Bearer"
            })
            .to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).await.unwrap();
        }
    });

    // Seed with a stale cached token whose expiry is in the past
    let stale_expiry = Instant::now() - Duration::from_secs(10);
    let source = GkeMetadataTokenSource::new()
        .with_endpoint(format!("http://127.0.0.1:{port}/token"))
        .with_cached_token("stale-cached-token", stale_expiry);

    let token = source.token().await.unwrap();
    assert_eq!(token, "fresh-token-from-server");
}

#[tokio::test]
async fn gke_metadata_token_source_handles_malformed_json_and_missing_or_invalid_fields() {
    let test_cases: Vec<(String, String)> = vec![
        (
            "SECRET_SENSITIVE_BODY_STRING_123".to_string(),
            "is not valid JSON".to_string(),
        ),
        (
            json!({ "expires_in": 3600 }).to_string(),
            "missing non-empty access_token".to_string(),
        ),
        (
            json!({ "access_token": "   ", "expires_in": 3600 }).to_string(),
            "missing non-empty access_token".to_string(),
        ),
        (
            json!({ "access_token": "SECRET_SENSITIVE_TOKEN_VALUE_XYZ123" }).to_string(),
            "missing valid positive integer expires_in".to_string(),
        ),
        (
            json!({ "access_token": "SECRET_SENSITIVE_TOKEN_VALUE_XYZ123", "expires_in": "3600" })
                .to_string(),
            "missing valid positive integer expires_in".to_string(),
        ),
        (
            json!({ "access_token": "SECRET_SENSITIVE_TOKEN_VALUE_XYZ123", "expires_in": 0 })
                .to_string(),
            "zero token lifetime".to_string(),
        ),
    ];

    for (body, expected_err_fragment) in test_cases {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let body_clone = body.clone();

        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            if let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body_clone.len(),
                    body_clone
                );
                stream.write_all(resp.as_bytes()).await.unwrap();
            }
        });

        let source =
            GkeMetadataTokenSource::new().with_endpoint(format!("http://127.0.0.1:{port}/token"));
        let err = source.token().await.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains(&expected_err_fragment),
            "expected error containing '{expected_err_fragment}', got '{err_msg}'"
        );
        // Ensure error message redacts sensitive token values and raw response body
        assert!(!err_msg.contains("SECRET_SENSITIVE_BODY_STRING_123"));
        assert!(!err_msg.contains("SECRET_SENSITIVE_TOKEN_VALUE_XYZ123"));
    }
}

#[tokio::test]
async fn gke_metadata_token_source_redacts_errors_on_failure() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();

    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        if let Ok((mut stream, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let body = "SENSITIVE_INTERNAL_SERVER_ERROR_BODY";
            let resp = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).await.unwrap();
        }
    });

    let source =
        GkeMetadataTokenSource::new().with_endpoint(format!("http://127.0.0.1:{port}/token"));
    let err = source.token().await.unwrap_err();
    let err_msg = err.to_string();
    assert!(!err_msg.contains("SENSITIVE_INTERNAL_SERVER_ERROR_BODY"));
    assert!(err_msg.contains("500"));
}
