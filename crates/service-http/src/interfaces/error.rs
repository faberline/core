//! The shared HTTP error envelope every k8s-native service in the
//! ecosystem renders for its error responses. lumen established this shape
//! first (a `StorageError` → status/kind classification over a
//! `{"error", "message"}` body); this module is the one place it lives so
//! `keep`/`relay`/`loom` converge on the same JSON instead of hand-rolling a
//! coincidentally-similar one. `crates/service-auth`'s own rejection
//! rendering predates this module and is a later convergence — untouched
//! here.
//!
//! [`error_code`] names the twelve catalog codes, and [`ErrorDetail`] is one
//! problem inside an error that [`ApiErr::with_details`] renders as a `details`
//! array in [`ErrorEnvelopeWithDetails`].
//!
//! [`ErrorEnvelope`] is the wire shape; [`ApiErr`] pairs a `StatusCode` with
//! a short machine-stable `kind` and a human `message`, and renders as
//! [`ErrorEnvelope`] JSON via `IntoResponse`. A service builds one per
//! domain-error classification arm ([`ApiErr::new`]) — this crate only owns
//! the generic envelope + builder, never the domain classification, which
//! stays in the service's own `From<DomainError>` impl.

use std::time::Duration;

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The twelve catalog error codes. Constants only: a service still picks the
/// `kind` of each error, and the existing `kind` strings do not change.
pub mod error_code {
    /// The request is malformed.
    pub const INVALID_REQUEST: &str = "invalid_request";
    /// The caller is not authenticated.
    pub const UNAUTHENTICATED: &str = "unauthenticated";
    /// The caller may not perform this action.
    pub const FORBIDDEN: &str = "forbidden";
    /// The resource does not exist.
    pub const NOT_FOUND: &str = "not_found";
    /// The request conflicts with the current state.
    pub const CONFLICT: &str = "conflict";
    /// A precondition of the request does not hold.
    pub const PRECONDITION_FAILED: &str = "precondition_failed";
    /// The request body is too large.
    pub const PAYLOAD_TOO_LARGE: &str = "payload_too_large";
    /// The request content failed validation.
    pub const VALIDATION_FAILED: &str = "validation_failed";
    /// A precondition header is required.
    pub const PRECONDITION_REQUIRED: &str = "precondition_required";
    /// The caller exceeded a rate limit.
    pub const RATE_LIMITED: &str = "rate_limited";
    /// An unexpected internal failure.
    pub const INTERNAL: &str = "internal";
    /// The service is unavailable, for example after a timeout.
    pub const UNAVAILABLE: &str = "unavailable";

    /// All twelve codes, in the order above.
    pub const ALL: [&str; 12] = [
        INVALID_REQUEST,
        UNAUTHENTICATED,
        FORBIDDEN,
        NOT_FOUND,
        CONFLICT,
        PRECONDITION_FAILED,
        PAYLOAD_TOO_LARGE,
        VALIDATION_FAILED,
        PRECONDITION_REQUIRED,
        RATE_LIMITED,
        INTERNAL,
        UNAVAILABLE,
    ];
}

/// One problem inside one error. `field` names the offending input and is left
/// out of the JSON when it is `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ErrorDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub code: String,
    pub message: String,
}

impl ErrorDetail {
    /// A problem with no field.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: None,
            code: code.into(),
            message: message.into(),
        }
    }

    /// Name the input field that this problem is about.
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
}

/// The `{error, message}` JSON body every ecosystem HTTP error response
/// renders. `error` is a short machine-stable classification
/// (`"not_found"`, `"bad_request"`, ...); `message` is the human-readable
/// detail.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorEnvelope {
    pub error: String,
    pub message: String,
}

/// Optional projection position attached to a detailed service error.
///
/// The shared HTTP layer owns the wire fields. The service still decides when
/// a projection is too far behind and which cursor values are meaningful.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProjectionMetadata {
    pub projection: String,
    pub required_cursor: u64,
    pub current_cursor: u64,
}

/// The additive error body used by services that publish retry information.
///
/// [`ErrorEnvelope`] remains the exact two-field compatibility shape. An
/// [`ApiErr`] renders this detailed form only after a detailed builder method
/// is called.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DetailedErrorEnvelope {
    pub error: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_cursor: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_cursor: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

/// The detailed error body plus an ordered `details` array.
///
/// [`ApiErr`] renders this shape only when [`ApiErr::with_details`] was given
/// a non-empty list. [`DetailedErrorEnvelope`] stays the shape of every other
/// detailed error, so its fields never change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ErrorEnvelopeWithDetails {
    pub error: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_cursor: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_cursor: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<ErrorDetail>,
}

/// Return the bounded client's requested delay for one detailed retryable
/// response.
///
/// A caller must not replay a request from a basic error body, a client error
/// other than `429`, or conflicting header and body metadata. The service
/// still decides whether replaying that request is safe and how many attempts
/// it permits.
pub fn retry_delay_from_detailed_error(
    status: StatusCode,
    headers: &HeaderMap,
    body: &[u8],
) -> Option<Duration> {
    if status != StatusCode::TOO_MANY_REQUESTS && !status.is_server_error() {
        return None;
    }
    let details: DetailedErrorEnvelope = serde_json::from_slice(body).ok()?;
    if !details.retryable {
        return None;
    }
    let header_seconds = match headers.get(header::RETRY_AFTER) {
        Some(value) => Some(value.to_str().ok()?.parse::<u64>().ok()?),
        None => None,
    };
    let seconds = match (header_seconds, details.retry_after_seconds) {
        (Some(header), Some(body)) if header == body => header,
        (None, None) => 0,
        _ => return None,
    };
    Some(Duration::from_secs(seconds))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DetailedErrorMetadata {
    retryable: bool,
    retry_after_seconds: Option<u64>,
    projection: Option<ProjectionMetadata>,
    error_details: Vec<ErrorDetail>,
}

/// Status-code + `kind` classification wrapper. Build one with
/// [`ApiErr::new`] from a domain-error match arm; `.into_response()` renders
/// it as [`ErrorEnvelope`] JSON paired with the status code.
pub struct ApiErr {
    status: StatusCode,
    kind: &'static str,
    message: String,
    details: Option<DetailedErrorMetadata>,
}

impl ApiErr {
    /// HTTP status + short machine-stable `kind` + human-readable `message`.
    pub fn new(status: StatusCode, kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            kind,
            message: message.into(),
            details: None,
        }
    }

    /// Select the detailed wire shape and state whether a caller may retry.
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.details
            .get_or_insert(DetailedErrorMetadata {
                retryable,
                retry_after_seconds: None,
                projection: None,
                error_details: Vec::new(),
            })
            .retryable = retryable;
        self
    }

    /// Attach a whole-second retry delay and render the matching Retry-After
    /// response header. A retry delay always makes the error retryable.
    pub fn with_retry_after_seconds(mut self, seconds: u64) -> Self {
        let details = self.details.get_or_insert(DetailedErrorMetadata {
            retryable: true,
            retry_after_seconds: None,
            projection: None,
            error_details: Vec::new(),
        });
        details.retryable = true;
        details.retry_after_seconds = Some(seconds);
        self
    }

    /// Attach a service-owned projection position to the shared detailed body.
    pub fn with_projection(mut self, projection: ProjectionMetadata) -> Self {
        self.details
            .get_or_insert(DetailedErrorMetadata {
                retryable: false,
                retry_after_seconds: None,
                projection: None,
                error_details: Vec::new(),
            })
            .projection = Some(projection);
        self
    }

    /// Attach the ordered problems of this error. Selects the detailed wire shape;
    /// an empty list renders no `details` key. The other builder methods keep
    /// working in any order.
    pub fn with_details(mut self, details: Vec<ErrorDetail>) -> Self {
        self.details
            .get_or_insert(DetailedErrorMetadata {
                retryable: false,
                retry_after_seconds: None,
                projection: None,
                error_details: Vec::new(),
            })
            .error_details = details;
        self
    }
}

impl IntoResponse for ApiErr {
    fn into_response(self) -> Response {
        let Some(details) = self.details else {
            return (
                self.status,
                Json(ErrorEnvelope {
                    error: self.kind.to_string(),
                    message: self.message,
                }),
            )
                .into_response();
        };

        let projection = details.projection;
        let retry_after_seconds = details.retry_after_seconds;
        let error_details = details.error_details;
        let error = self.kind.to_string();
        let projection_name = projection.as_ref().map(|value| value.projection.clone());
        let required_cursor = projection.as_ref().map(|value| value.required_cursor);
        let current_cursor = projection.as_ref().map(|value| value.current_cursor);
        let mut response = if error_details.is_empty() {
            (
                self.status,
                Json(DetailedErrorEnvelope {
                    error,
                    message: self.message,
                    retryable: details.retryable,
                    projection: projection_name,
                    required_cursor,
                    current_cursor,
                    retry_after_seconds,
                }),
            )
                .into_response()
        } else {
            (
                self.status,
                Json(ErrorEnvelopeWithDetails {
                    error,
                    message: self.message,
                    retryable: details.retryable,
                    projection: projection_name,
                    required_cursor,
                    current_cursor,
                    retry_after_seconds,
                    details: error_details,
                }),
            )
                .into_response()
        };
        if let Some(seconds) = retry_after_seconds {
            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn renders_status_and_envelope_json() {
        let resp = ApiErr::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "collection not found: u",
        )
        .into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"], "not_found");
        assert_eq!(body["message"], "collection not found: u");
    }

    #[test]
    fn envelope_serializes_error_before_message() {
        let env = ErrorEnvelope {
            error: "bad_request".into(),
            message: "oops".into(),
        };
        let json = serde_json::to_string(&env).unwrap();
        assert_eq!(json, r#"{"error":"bad_request","message":"oops"}"#);
    }

    #[test]
    fn envelope_round_trips_through_deserialize() {
        let raw = r#"{"error":"gone","message":"tombstoned"}"#;
        let env: ErrorEnvelope = serde_json::from_str(raw).unwrap();
        assert_eq!(env.error, "gone");
        assert_eq!(env.message, "tombstoned");
    }
}
