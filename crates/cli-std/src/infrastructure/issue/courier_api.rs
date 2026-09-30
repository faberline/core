use serde_json::Value;

use crate::domain::issue::created::{created_issue_from_response, CreatedIssue};
use crate::domain::issue::payload::comment_payload;
use crate::domain::issue::tracker::CourierApi;
use crate::domain::remote::RemoteError;
use crate::infrastructure::courier::resolve_courier_token;
use crate::infrastructure::github::parse_json;
use crate::infrastructure::http::HttpClient;

impl CourierApi for HttpClient {
    async fn courier_get_json(&self, url: &str, what: &'static str) -> Result<Value, RemoteError> {
        parse_json(courier_get(&self.client, url).await?, what).await
    }

    /// `POST /v1/issues/{owner}/{name}` via courier — same response shape as
    /// `submit_issue()` (GitHub's created-issue JSON, forwarded verbatim).
    async fn submit_issue_via_courier(
        &self,
        url: &str,
        payload: &Value,
    ) -> Result<CreatedIssue, RemoteError> {
        let resp = courier_post(&self.client, url, payload).await?;
        let value = parse_json(resp, "courier issue response").await?;
        Ok(created_issue_from_response(&value))
    }

    /// `POST /v1/issues/{owner}/{name}/{number}/comments` via courier — courier
    /// reopens the issue server-side then creates the comment in one round
    /// trip, returning the created-comment JSON (same shape as
    /// `post_issue_comment()`'s response).
    async fn post_issue_comment_via_courier(
        &self,
        url: &str,
        body: &str,
    ) -> Result<String, RemoteError> {
        let resp = courier_post(&self.client, url, &comment_payload(body)).await?;
        let value = parse_json(resp, "courier comment response").await?;
        Ok(value
            .get("html_url")
            .and_then(|u| u.as_str())
            .unwrap_or("(comment created)")
            .to_string())
    }
}

/// `GET` against courier, authenticated with `resolve_courier_token()` (the
/// courier bearer token, not a GitHub token) — mirrors `github_get()`.
async fn courier_get(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, RemoteError> {
    let mut req = client.get(url);
    if let Some(token) = resolve_courier_token() {
        req = req.bearer_auth(token);
    }
    send(req, "GET", url).await
}

/// `POST` against courier with a JSON body, authenticated with
/// `resolve_courier_token()`.
async fn courier_post(
    client: &reqwest::Client,
    url: &str,
    payload: &Value,
) -> Result<reqwest::Response, RemoteError> {
    let mut req = client.post(url).json(payload);
    if let Some(token) = resolve_courier_token() {
        req = req.bearer_auth(token);
    }
    send(req, "POST", url).await
}

/// Send a courier request, failing on an error status.
async fn send(
    req: reqwest::RequestBuilder,
    method: &'static str,
    url: &str,
) -> Result<reqwest::Response, RemoteError> {
    req.send()
        .await
        .map_err(|e| RemoteError::Send {
            method,
            target: url.to_string(),
            source: Box::new(e),
        })?
        .error_for_status()
        .map_err(|e| RemoteError::Status {
            service: "courier",
            url: url.to_string(),
            source: Box::new(e),
        })
}
