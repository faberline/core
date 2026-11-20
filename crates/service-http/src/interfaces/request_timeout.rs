//! Opt-in request timeout. When the inner service does not answer within the limit, the
//! response is `503` with the catalog code `unavailable` and a message that names the timeout.
//!
//! Wire it with `Router::layer(axum::middleware::from_fn_with_state(RequestTimeout::standard(),
//! request_timeout_middleware))`. The standard limit is 30 seconds. The limit covers the time
//! until the inner service returns its response (its headers), not a streamed body. A zero limit
//! times out any inner service that does not answer at once. The dropped handler future is
//! cancelled.

use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::ApiErr;

/// The request time limit of [`request_timeout_middleware`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestTimeout {
    limit: Duration,
}

impl RequestTimeout {
    /// The standard limit: 30 seconds.
    pub const STANDARD_LIMIT: Duration = Duration::from_secs(30);

    /// A timeout with the given limit.
    pub fn new(limit: Duration) -> Self {
        Self { limit }
    }

    /// A timeout with the standard limit of 30 seconds.
    pub fn standard() -> Self {
        Self::new(Self::STANDARD_LIMIT)
    }

    /// The limit.
    pub fn limit(&self) -> Duration {
        self.limit
    }
}

/// Answer `503 unavailable` when the inner service does not return a response within the limit
/// of `timeout`. A fast handler is not changed.
pub async fn request_timeout_middleware(
    State(timeout): State<RequestTimeout>,
    request: Request,
    next: Next,
) -> Response {
    match tokio::time::timeout(timeout.limit, next.run(request)).await {
        Ok(response) => response,
        Err(_) => ApiErr::new(
            StatusCode::SERVICE_UNAVAILABLE,
            crate::interfaces::error_code::UNAVAILABLE,
            format!("request timed out after {} ms", timeout.limit.as_millis()),
        )
        .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_limit_is_thirty_seconds() {
        assert_eq!(RequestTimeout::standard().limit(), Duration::from_secs(30));
        assert_eq!(
            RequestTimeout::new(Duration::from_millis(5)).limit(),
            Duration::from_millis(5)
        );
    }
}
