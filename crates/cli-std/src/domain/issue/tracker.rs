//! How the issue verbs reach the tracker: straight to the GitHub API, or
//! through the courier proxy when one is configured.

use serde_json::Value;

use super::created::CreatedIssue;
use crate::domain::remote::RemoteError;

/// Which way the verbs reach the tracker. Each value is read when a verb
/// needs it, not before.
pub(crate) trait TrackerAccess {
    /// The courier proxy URL. When it is set, every verb goes through
    /// courier.
    fn courier_url(&self) -> Option<String>;
    /// A GitHub token for the direct path, if one is available.
    fn github_token(&self) -> Option<String>;
}

/// The GitHub REST API.
pub(crate) trait GitHubApi {
    /// GET `url` and parse its JSON body. `what` names the body in a parse
    /// error.
    async fn get_json(&self, url: &str, what: &'static str) -> Result<Value, RemoteError>;
    /// File an issue in `repo`.
    async fn submit_issue(
        &self,
        repo: &str,
        token: &str,
        payload: &Value,
    ) -> Result<CreatedIssue, RemoteError>;
    /// Reopen issue `number`; returns its URL.
    async fn reopen_issue(
        &self,
        repo: &str,
        number: u64,
        token: &str,
    ) -> Result<String, RemoteError>;
    /// Comment on issue `number`; returns the comment's URL.
    async fn post_issue_comment(
        &self,
        repo: &str,
        number: u64,
        token: &str,
        body: &str,
    ) -> Result<String, RemoteError>;
}

/// courier's `/v1/issues/...` endpoints. The URLs come from the builders in
/// `domain::issue::url`.
pub(crate) trait CourierApi {
    /// GET `url` and parse its JSON body. `what` names the body in a parse
    /// error.
    async fn courier_get_json(&self, url: &str, what: &'static str) -> Result<Value, RemoteError>;
    /// File an issue through courier's create endpoint `url`.
    async fn submit_issue_via_courier(
        &self,
        url: &str,
        payload: &Value,
    ) -> Result<CreatedIssue, RemoteError>;
    /// Comment through courier's comment endpoint `url`; courier reopens the
    /// issue first. Returns the comment's URL.
    async fn post_issue_comment_via_courier(
        &self,
        url: &str,
        body: &str,
    ) -> Result<String, RemoteError>;
}

/// The status line of a running node that a report can include.
pub(crate) trait NodeProbe {
    /// A one-line summary of the node's `/version` and `/healthz`, or a note
    /// that it is unreachable.
    async fn node_status(&self, url: &str) -> String;
}
