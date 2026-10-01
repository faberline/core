use anyhow::Result;

use crate::ToolInfo;

#[cfg(feature = "online")]
use super::repo::split_repo_owner_name;
#[cfg(feature = "online")]
use crate::domain::{
    issue::{
        tracker::{CourierApi, GitHubApi, TrackerAccess},
        url::{courier_view_url, github_view_url},
    },
    remote::RemoteError,
};

/// `issue view` — print a single issue by number. `open` builds the HTTP
/// client; `access` says whether to go through courier.
#[cfg(feature = "online")]
pub(crate) async fn view<A>(
    tool: &ToolInfo,
    number: u64,
    access: &impl TrackerAccess,
    open: impl FnOnce() -> Result<A, RemoteError>,
) -> Result<()>
where
    A: GitHubApi + CourierApi,
{
    let api = open()?;

    let v: serde_json::Value = if let Some(courier_url) = access.courier_url() {
        let (owner, name) = split_repo_owner_name(tool.repo())?;
        let url = courier_view_url(&courier_url, owner, name, number);
        api.courier_get_json(&url, "courier issue response").await?
    } else {
        let url = github_view_url(tool.repo(), number);
        api.get_json(&url, "issue response").await?
    };

    let state = v.get("state").and_then(|s| s.as_str()).unwrap_or("?");
    let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("");
    let html = v.get("html_url").and_then(|u| u.as_str()).unwrap_or("");
    let body = v.get("body").and_then(|b| b.as_str()).unwrap_or("");
    println!("#{number} [{state}] {title}");
    if !html.is_empty() {
        println!("{html}");
    }
    println!("---");
    println!(
        "{}",
        if body.trim().is_empty() {
            "(no description)"
        } else {
            body
        }
    );
    println!("next: done");
    Ok(())
}

#[cfg(not(feature = "online"))]
pub async fn view(_tool: &ToolInfo, _number: u64) -> Result<()> {
    anyhow::bail!("this build has no `online` feature — `issue view` needs network access")
}
