//! `If-Match` precondition check for write requests (RFC 9110 strong comparison).

use crate::interfaces::{error_code, ApiErr};
use axum::http::{header, HeaderMap, Method, StatusCode};

/// Check the `If-Match` header of a request against the current ETag of the resource.
// `ApiErr` is the crate-wide error type; the signature stays as the spec gives it.
#[allow(clippy::result_large_err)]
pub fn check_if_match(
    method: &Method,
    headers: &HeaderMap,
    current_etag: &str,
) -> Result<(), ApiErr> {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return Ok(());
    }
    let Some(raw) = headers.get(header::IF_MATCH) else {
        return Err(ApiErr::new(
            StatusCode::PRECONDITION_REQUIRED,
            error_code::PRECONDITION_REQUIRED,
            "If-Match header is required for this request",
        ));
    };
    let mismatch = || {
        ApiErr::new(
            StatusCode::PRECONDITION_FAILED,
            error_code::PRECONDITION_FAILED,
            "If-Match does not match the current ETag",
        )
    };
    let Ok(value) = raw.to_str() else {
        return Err(mismatch());
    };
    let value = value.trim();
    if value == "*" {
        return Ok(());
    }
    let matched = value
        .split(',')
        .map(str::trim)
        .any(|tag| tag == current_etag && !tag.starts_with("W/"));
    if matched {
        Ok(())
    } else {
        Err(mismatch())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{header, HeaderValue, StatusCode};
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    fn headers_with_if_match(value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_MATCH, HeaderValue::from_static(value));
        headers
    }

    async fn status_and_body(err: ApiErr) -> (StatusCode, serde_json::Value) {
        let response = err.into_response();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn matching_tag_passes() {
        let headers = headers_with_if_match("\"v2\"");
        assert!(check_if_match(&Method::PUT, &headers, "\"v2\"").is_ok());
        let headers = headers_with_if_match("\"v1\", \"v2\"");
        assert!(check_if_match(&Method::PUT, &headers, "\"v2\"").is_ok());
        let headers = headers_with_if_match("*");
        assert!(check_if_match(&Method::PUT, &headers, "\"v2\"").is_ok());
    }

    #[tokio::test]
    async fn stale_tag_is_412_precondition_failed() {
        let headers = headers_with_if_match("\"v1\"");
        let err = check_if_match(&Method::PUT, &headers, "\"v2\"").unwrap_err();
        let (status, body) = status_and_body(err).await;
        assert_eq!(status, StatusCode::PRECONDITION_FAILED);
        assert_eq!(body["error"], "precondition_failed");

        // A weak tag never matches under the strong comparison.
        let headers = headers_with_if_match("W/\"v2\"");
        let err = check_if_match(&Method::PUT, &headers, "\"v2\"").unwrap_err();
        assert_eq!(
            status_and_body(err).await.0,
            StatusCode::PRECONDITION_FAILED
        );
    }

    #[tokio::test]
    async fn missing_header_on_write_is_428_precondition_required() {
        for method in [Method::PUT, Method::DELETE] {
            let err = check_if_match(&method, &HeaderMap::new(), "\"v2\"").unwrap_err();
            let (status, body) = status_and_body(err).await;
            assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
            assert_eq!(body["error"], "precondition_required");
        }
    }

    #[tokio::test]
    async fn read_needs_no_header() {
        for method in [Method::GET, Method::HEAD, Method::OPTIONS] {
            assert!(check_if_match(&method, &HeaderMap::new(), "\"v2\"").is_ok());
        }
        let headers = headers_with_if_match("\"v1\"");
        assert!(check_if_match(&Method::GET, &headers, "\"v2\"").is_ok());
    }
}
