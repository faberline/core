use std::collections::HashMap;

use super::*;

// -- MetadataTokenSource -----------------------------------------------

#[tokio::test]
async fn metadata_token_source_sends_header_and_query_and_returns_token() {
    use axum::{extract::Query, http::HeaderMap, routing::get, Router};

    let app = Router::new().route(
        "/computeMetadata/v1/instance/service-accounts/default/identity",
        get(
            |headers: HeaderMap, Query(params): Query<HashMap<String, String>>| async move {
                assert_eq!(
                    headers.get("Metadata-Flavor").and_then(|v| v.to_str().ok()),
                    Some("Google")
                );
                assert_eq!(params.get("audience").map(String::as_str), Some("test-aud"));
                assert_eq!(params.get("format").map(String::as_str), Some("full"));
                "eyJhbGciOiJSUzI1NiJ9.test.jwt"
            },
        ),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let source = MetadataTokenSource::new(client, &base_url);
    let token = source.fetch_id_token("test-aud").await.unwrap();
    assert_eq!(token, "eyJhbGciOiJSUzI1NiJ9.test.jwt");
}

#[tokio::test]
async fn metadata_token_source_surfaces_typed_error_naming_base_url() {
    use axum::{routing::get, Router};

    let app = Router::new().route(
        "/computeMetadata/v1/instance/service-accounts/default/identity",
        get(|| async { (axum::http::StatusCode::NOT_FOUND, "not on GCP") }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let source = MetadataTokenSource::new(client, &base_url);
    let err = source.fetch_id_token("test-aud").await.unwrap_err();
    assert!(
        err.to_string().contains(&base_url),
        "error message {err} should name base_url {base_url}"
    );
    assert!(matches!(
        err,
        MetadataTokenError::HttpStatus {
            status: axum::http::StatusCode::NOT_FOUND,
            ..
        }
    ));
}
