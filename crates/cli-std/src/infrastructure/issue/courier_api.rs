use anyhow::Result;

use crate::domain::issue::created::{created_issue_from_response, CreatedIssue};
use crate::domain::issue::payload::comment_payload;

/// `GET` against courier, authenticated with `resolve_courier_token()` (the
/// courier bearer token, not a GitHub token) — mirrors `crate::github_get()`.
#[cfg(feature = "online")]
pub(crate) async fn courier_get(client: &reqwest::Client, url: &str) -> Result<reqwest::Response> {
    use anyhow::Context;
    let mut req = client.get(url);
    if let Some(token) = crate::resolve_courier_token() {
        req = req.bearer_auth(token);
    }
    req.send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("courier error for {url}"))
}

/// `POST` against courier with a JSON body, authenticated with
/// `resolve_courier_token()`.
#[cfg(feature = "online")]
async fn courier_post(
    client: &reqwest::Client,
    url: &str,
    payload: &serde_json::Value,
) -> Result<reqwest::Response> {
    use anyhow::Context;
    let mut req = client.post(url).json(payload);
    if let Some(token) = crate::resolve_courier_token() {
        req = req.bearer_auth(token);
    }
    req.send()
        .await
        .with_context(|| format!("POST {url}"))?
        .error_for_status()
        .with_context(|| format!("courier error for {url}"))
}

/// `POST /v1/issues/{owner}/{name}` via courier — same response shape as
/// `submit_issue()` (GitHub's created-issue JSON, forwarded verbatim).
#[cfg(feature = "online")]
pub(crate) async fn submit_issue_via_courier(
    client: &reqwest::Client,
    url: &str,
    payload: &serde_json::Value,
) -> Result<CreatedIssue> {
    use anyhow::Context;
    let value: serde_json::Value = courier_post(client, url, payload)
        .await?
        .json()
        .await
        .context("parse courier issue response")?;
    Ok(created_issue_from_response(&value))
}

/// `POST /v1/issues/{owner}/{name}/{number}/comments` via courier — courier
/// reopens the issue server-side then creates the comment in one round
/// trip, returning the created-comment JSON (same shape as
/// `post_issue_comment()`'s response).
#[cfg(feature = "online")]
pub(crate) async fn post_issue_comment_via_courier(
    client: &reqwest::Client,
    url: &str,
    body: &str,
) -> Result<String> {
    use anyhow::Context;
    let value: serde_json::Value = courier_post(client, url, &comment_payload(body))
        .await?
        .json()
        .await
        .context("parse courier comment response")?;
    Ok(value
        .get("html_url")
        .and_then(|u| u.as_str())
        .unwrap_or("(comment created)")
        .to_string())
}
