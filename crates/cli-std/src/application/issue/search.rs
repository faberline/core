use anyhow::Result;

use crate::ToolInfo;

#[cfg(feature = "online")]
use super::repo::split_repo_owner_name;
#[cfg(feature = "online")]
use crate::domain::{
    issue::{
        tracker::{CourierApi, GitHubApi, TrackerAccess},
        url::{courier_search_url, github_search_url},
    },
    remote::RemoteError,
};

/// Flags for `issue search`.
#[derive(Clone, Debug)]
pub struct SearchOptions {
    /// Free-text query; `None`/empty lists recent issues for this tool.
    pub query: Option<String>,
    /// `open` (default), `closed`, or `all`.
    pub state: String,
    /// Max results.
    pub limit: u32,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            query: None,
            state: "open".to_string(),
            limit: 20,
        }
    }
}

/// `issue search` — list/search this tool's issues (filtered to
/// `app:<name>`). `open` builds the HTTP client; `access` says whether to go
/// through courier.
#[cfg(feature = "online")]
pub(crate) async fn search<A>(
    tool: &ToolInfo,
    opts: SearchOptions,
    access: &impl TrackerAccess,
    open: impl FnOnce() -> Result<A, RemoteError>,
) -> Result<()>
where
    A: GitHubApi + CourierApi,
{
    let label = tool.issue_label();
    let api = open()?;

    let v: serde_json::Value = if let Some(courier_url) = access.courier_url() {
        let (owner, name) = split_repo_owner_name(tool.repo)?;
        let mut q = format!("label:\"{label}\"");
        if let Some(text) = opts.query.as_deref() {
            if !text.trim().is_empty() {
                q.push(' ');
                q.push_str(text.trim());
            }
        }
        let url = courier_search_url(&courier_url, owner, name, &opts.state, &q, opts.limit);
        api.courier_get_json(&url, "courier issue search response")
            .await?
    } else {
        let mut q = format!("repo:{} is:issue label:\"{}\"", tool.repo, label);
        if opts.state != "all" {
            q.push_str(&format!(" state:{}", opts.state));
        }
        if let Some(text) = opts.query.as_deref() {
            if !text.trim().is_empty() {
                q.push(' ');
                q.push_str(text.trim());
            }
        }
        let url = github_search_url(&q, opts.limit);
        api.get_json(&url, "issue search response").await?
    };

    let items = v.get("items").and_then(|i| i.as_array());
    match items {
        Some(items) if !items.is_empty() => {
            for it in items {
                let num = it.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
                let state = it.get("state").and_then(|s| s.as_str()).unwrap_or("?");
                let title = it.get("title").and_then(|t| t.as_str()).unwrap_or("");
                println!("#{num} [{state}] {title}");
            }
        }
        _ => println!("no {label} issues match"),
    }
    println!("next: done");
    Ok(())
}

#[cfg(not(feature = "online"))]
pub async fn search(_tool: &ToolInfo, _opts: SearchOptions) -> Result<()> {
    anyhow::bail!("this build has no `online` feature — `issue search` needs network access")
}
