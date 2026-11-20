// HANDWRITE-BEGIN gap="missing-generator:unit-test:5d2f7c13" tracker="#562" reason="Live coverage for catalog error codes, the details array wire form and the request timeout middleware."
use std::collections::BTreeSet;

use std::time::Duration;

use axum::{
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use service_http::{
    error_code, retry_delay_from_detailed_error, ApiErr, DetailedErrorEnvelope, ErrorDetail,
    ErrorEnvelopeWithDetails,
};

async fn body_bytes(response: Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

#[test]
fn catalog_has_twelve_public_codes() {
    assert_eq!(
        error_code::ALL,
        [
            "invalid_request",
            "unauthenticated",
            "forbidden",
            "not_found",
            "conflict",
            "precondition_failed",
            "payload_too_large",
            "validation_failed",
            "precondition_required",
            "rate_limited",
            "internal",
            "unavailable",
        ]
    );
    assert_eq!(error_code::INVALID_REQUEST, "invalid_request");
    assert_eq!(error_code::UNAUTHENTICATED, "unauthenticated");
    assert_eq!(error_code::FORBIDDEN, "forbidden");
    assert_eq!(error_code::NOT_FOUND, "not_found");
    assert_eq!(error_code::CONFLICT, "conflict");
    assert_eq!(error_code::PRECONDITION_FAILED, "precondition_failed");
    assert_eq!(error_code::PAYLOAD_TOO_LARGE, "payload_too_large");
    assert_eq!(error_code::VALIDATION_FAILED, "validation_failed");
    assert_eq!(error_code::PRECONDITION_REQUIRED, "precondition_required");
    assert_eq!(error_code::RATE_LIMITED, "rate_limited");
    assert_eq!(error_code::INTERNAL, "internal");
    assert_eq!(error_code::UNAVAILABLE, "unavailable");
    let distinct: BTreeSet<&str> = error_code::ALL.into_iter().collect();
    assert_eq!(distinct.len(), 12);
}

#[tokio::test]
async fn error_without_details_is_byte_equal() {
    let basic = ApiErr::new(StatusCode::BAD_REQUEST, "bad_request", "bad input").into_response();
    assert_eq!(
        body_bytes(basic).await,
        br#"{"error":"bad_request","message":"bad input"}"#
    );

    let detailed = ApiErr::new(StatusCode::SERVICE_UNAVAILABLE, "overloaded", "busy")
        .with_retryable(true)
        .into_response();
    assert_eq!(
        body_bytes(detailed).await,
        br#"{"error":"overloaded","message":"busy","retryable":true}"#
    );
}

#[tokio::test]
async fn two_details_render_in_order_and_field_is_omitted_when_none() {
    let response = ApiErr::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        error_code::VALIDATION_FAILED,
        "validation failed",
    )
    .with_details(vec![
        ErrorDetail::new("required", "name is required").with_field("name"),
        ErrorDetail::new("invalid", "bad value"),
    ])
    .into_response();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body_bytes(response).await,
        br#"{"error":"validation_failed","message":"validation failed","retryable":false,"details":[{"field":"name","code":"required","message":"name is required"},{"code":"invalid","message":"bad value"}]}"#
    );
}

#[tokio::test]
async fn details_combine_with_retry_after() {
    let response = ApiErr::new(
        StatusCode::TOO_MANY_REQUESTS,
        error_code::RATE_LIMITED,
        "slow down",
    )
    .with_details(vec![ErrorDetail::new("quota", "daily quota used")])
    .with_retry_after_seconds(2)
    .into_response();
    assert_eq!(response.headers().get(header::RETRY_AFTER).unwrap(), "2");
    let body: serde_json::Value = serde_json::from_slice(&body_bytes(response).await).unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "error": "rate_limited",
            "message": "slow down",
            "retryable": true,
            "retry_after_seconds": 2,
            "details": [{"code": "quota", "message": "daily quota used"}],
        })
    );
}

#[tokio::test]
async fn empty_details_list_renders_no_details_key() {
    let response = ApiErr::new(StatusCode::BAD_REQUEST, error_code::INVALID_REQUEST, "bad")
        .with_details(Vec::new())
        .into_response();
    assert_eq!(
        body_bytes(response).await,
        br#"{"error":"invalid_request","message":"bad","retryable":false}"#
    );
}

#[test]
fn detailed_envelope_keeps_its_seven_fields() {
    // A struct literal without `..`: adding a field to DetailedErrorEnvelope
    // stops this test from compiling, which guards the semver gate.
    let value = DetailedErrorEnvelope {
        error: "e".into(),
        message: "m".into(),
        retryable: false,
        projection: None,
        required_cursor: None,
        current_cursor: None,
        retry_after_seconds: None,
    };
    assert_eq!(
        serde_json::to_string(&value).unwrap(),
        r#"{"error":"e","message":"m","retryable":false}"#
    );
}

#[test]
fn envelope_with_details_deserializes_with_and_without_the_key() {
    let bare: ErrorEnvelopeWithDetails =
        serde_json::from_str(r#"{"error":"e","message":"m","retryable":false}"#).unwrap();
    assert!(bare.details.is_empty());

    let with: ErrorEnvelopeWithDetails = serde_json::from_str(
        r#"{"error":"e","message":"m","retryable":false,"details":[{"code":"c","message":"x"}]}"#,
    )
    .unwrap();
    assert_eq!(with.details.len(), 1);
    assert_eq!(with.details[0].code, "c");
    assert_eq!(with.details[0].field, None);
}

#[test]
fn retry_delay_ignores_the_details_key() {
    let mut headers = HeaderMap::new();
    headers.insert(header::RETRY_AFTER, HeaderValue::from_static("2"));
    let body = br#"{"error":"unavailable","message":"m","retryable":true,"retry_after_seconds":2,"details":[{"code":"c","message":"x"}]}"#;
    assert_eq!(
        retry_delay_from_detailed_error(StatusCode::SERVICE_UNAVAILABLE, &headers, body),
        Some(Duration::from_secs(2))
    );
}
// HANDWRITE-END
