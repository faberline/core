use serde_json::Value;

use crate::domain::issue::created::{created_issue_from_response, CreatedIssue};
use crate::domain::issue::payload::{comment_payload, reopen_payload};
use crate::domain::issue::tracker::GitHubApi;
use crate::domain::issue::url::issue_url;
use crate::domain::remote::RemoteError;
use crate::infrastructure::github::{github_get, parse_json};
use crate::infrastructure::http::HttpClient;

impl GitHubApi for HttpClient {
    async fn get_json(&self, url: &str, what: &'static str) -> Result<Value, RemoteError> {
        parse_json(github_get(&self.client, url).await?, what).await
    }

    async fn submit_issue(
        &self,
        repo: &str,
        token: &str,
        payload: &Value,
    ) -> Result<CreatedIssue, RemoteError> {
        let url = format!("https://api.github.com/repos/{repo}/issues");
        let request = Write {
            method: "POST",
            target: "issue",
            what: "issue response",
        };
        let value = request.send(self.client.post(&url), token, payload).await?;
        Ok(created_issue_from_response(&value))
    }

    async fn reopen_issue(
        &self,
        repo: &str,
        number: u64,
        token: &str,
    ) -> Result<String, RemoteError> {
        let url = format!("https://api.github.com/repos/{repo}/issues/{number}");
        let request = Write {
            method: "PATCH",
            target: "issue",
            what: "issue response",
        };
        let value = request
            .send(self.client.patch(&url), token, &reopen_payload())
            .await?;
        Ok(value
            .get("html_url")
            .and_then(|u| u.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| issue_url(repo, number)))
    }

    async fn post_issue_comment(
        &self,
        repo: &str,
        number: u64,
        token: &str,
        body: &str,
    ) -> Result<String, RemoteError> {
        let url = format!("https://api.github.com/repos/{repo}/issues/{number}/comments");
        let request = Write {
            method: "POST",
            target: "issue comment",
            what: "comment response",
        };
        let value = request
            .send(self.client.post(&url), token, &comment_payload(body))
            .await?;
        Ok(value
            .get("html_url")
            .and_then(|u| u.as_str())
            .unwrap_or("(comment created)")
            .to_string())
    }
}

/// An authenticated write to the GitHub API, named for its errors: `method`
/// and `target` for a send failure, `what` for a body that does not parse.
struct Write {
    method: &'static str,
    target: &'static str,
    what: &'static str,
}

impl Write {
    /// Send `payload` and parse the JSON answer. The body is parsed before the
    /// status is checked, since a refusal carries its reason in the body's
    /// `message`.
    async fn send(
        self,
        req: reqwest::RequestBuilder,
        token: &str,
        payload: &Value,
    ) -> Result<Value, RemoteError> {
        let resp = req
            .header("Accept", "application/vnd.github+json")
            .bearer_auth(token)
            .json(payload)
            .send()
            .await
            .map_err(|e| RemoteError::Send {
                method: self.method,
                target: self.target.to_string(),
                source: Box::new(e),
            })?;
        let status = resp.status();
        let value = parse_json(resp, self.what).await?;
        if !status.is_success() {
            let message = value
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(RemoteError::Rejected {
                status: status.to_string(),
                message: message.to_string(),
            });
        }
        Ok(value)
    }
}
