use anyhow::Result;

use crate::ToolInfo;

#[cfg(feature = "online")]
use super::{client::http_client, repo::split_repo_owner_name};
#[cfg(feature = "online")]
use crate::{
    domain::issue::url::{courier_view_url, github_view_url},
    infrastructure::issue::courier_api::courier_get,
};

/// `issue view` — print a single issue by number.
#[cfg(feature = "online")]
pub async fn view(tool: &ToolInfo, number: u64) -> Result<()> {
    use anyhow::Context;
    let client = http_client(tool)?;

    let v: serde_json::Value = if let Some(courier_url) = crate::resolve_courier_url() {
        let (owner, name) = split_repo_owner_name(tool.repo)?;
        let url = courier_view_url(&courier_url, owner, name, number);
        courier_get(&client, &url)
            .await?
            .json()
            .await
            .context("parse courier issue response")?
    } else {
        let url = github_view_url(tool.repo, number);
        crate::github_get(&client, &url)
            .await?
            .json()
            .await
            .context("parse issue response")?
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
