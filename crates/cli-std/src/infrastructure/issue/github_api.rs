use anyhow::Result;

use crate::domain::issue::created::{created_issue_from_response, CreatedIssue};
use crate::domain::issue::payload::{comment_payload, reopen_payload};
use crate::domain::issue::url::issue_url;

#[cfg(feature = "online")]
pub(crate) async fn submit_issue(
    client: &reqwest::Client,
    repo: &str,
    token: &str,
    payload: &serde_json::Value,
) -> Result<CreatedIssue> {
    use anyhow::{bail, Context};
    let url = format!("https://api.github.com/repos/{repo}/issues");
    let resp = client
        .post(&url)
        .header("Accept", "application/vnd.github+json")
        .bearer_auth(token)
        .json(payload)
        .send()
        .await
        .context("POST issue")?;
    let status = resp.status();
    let value: serde_json::Value = resp.json().await.context("parse issue response")?;
    if !status.is_success() {
        let msg = value
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        bail!("GitHub returned {status}: {msg}");
    }
    Ok(created_issue_from_response(&value))
}

#[cfg(feature = "online")]
pub(crate) async fn reopen_issue(
    client: &reqwest::Client,
    repo: &str,
    number: u64,
    token: &str,
) -> Result<String> {
    use anyhow::{bail, Context};
    let url = format!("https://api.github.com/repos/{repo}/issues/{number}");
    let resp = client
        .patch(&url)
        .header("Accept", "application/vnd.github+json")
        .bearer_auth(token)
        .json(&reopen_payload())
        .send()
        .await
        .context("PATCH issue")?;
    let status = resp.status();
    let value: serde_json::Value = resp.json().await.context("parse issue response")?;
    if !status.is_success() {
        let msg = value
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        bail!("GitHub returned {status}: {msg}");
    }
    Ok(value
        .get("html_url")
        .and_then(|u| u.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| issue_url(repo, number)))
}

#[cfg(feature = "online")]
pub(crate) async fn post_issue_comment(
    client: &reqwest::Client,
    repo: &str,
    number: u64,
    token: &str,
    body: &str,
) -> Result<String> {
    use anyhow::{bail, Context};
    let url = format!("https://api.github.com/repos/{repo}/issues/{number}/comments");
    let resp = client
        .post(&url)
        .header("Accept", "application/vnd.github+json")
        .bearer_auth(token)
        .json(&comment_payload(body))
        .send()
        .await
        .context("POST issue comment")?;
    let status = resp.status();
    let value: serde_json::Value = resp.json().await.context("parse comment response")?;
    if !status.is_success() {
        let msg = value
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        bail!("GitHub returned {status}: {msg}");
    }
    Ok(value
        .get("html_url")
        .and_then(|u| u.as_str())
        .unwrap_or("(comment created)")
        .to_string())
}
