use anyhow::Result;

use crate::ToolInfo;

#[cfg(feature = "online")]
use super::{client::http_client, repo::split_repo_owner_name};
#[cfg(feature = "online")]
use crate::{
    domain::issue::url::{courier_search_url, github_search_url},
    infrastructure::issue::courier_api::courier_get,
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

/// `issue search` — list/search this tool's issues (filtered to `app:<name>`).
#[cfg(feature = "online")]
pub async fn search(tool: &ToolInfo, opts: SearchOptions) -> Result<()> {
    use anyhow::Context;
    let label = tool.issue_label();
    let client = http_client(tool)?;

    let v: serde_json::Value = if let Some(courier_url) = crate::resolve_courier_url() {
        let (owner, name) = split_repo_owner_name(tool.repo)?;
        let mut q = format!("label:\"{label}\"");
        if let Some(text) = opts.query.as_deref() {
            if !text.trim().is_empty() {
                q.push(' ');
                q.push_str(text.trim());
            }
        }
        let url = courier_search_url(&courier_url, owner, name, &opts.state, &q, opts.limit);
        courier_get(&client, &url)
            .await?
            .json()
            .await
            .context("parse courier issue search response")?
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
        crate::github_get(&client, &url)
            .await?
            .json()
            .await
            .context("parse issue search response")?
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
