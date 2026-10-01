use anyhow::Result;

use super::diagnostics::render_diagnostics;
use super::labels::report_labels;
use super::output::{print_fallback, print_preview};
use super::repo::resolve_repo;
use crate::domain::issue::body::assemble_body;
use crate::ToolInfo;

#[cfg(not(feature = "online"))]
use super::output::note_offline_build;
#[cfg(feature = "online")]
use super::{
    client::http_client,
    output::{note_no_credential, print_created_issue},
    repo::split_repo_owner_name,
};
#[cfg(feature = "online")]
use crate::{
    domain::issue::{payload::issue_payload, url::courier_create_url},
    infrastructure::issue::{
        courier_api::submit_issue_via_courier, github_api::submit_issue,
        node_status::fetch_node_status,
    },
};

/// Flags for `issue create`.
#[derive(Clone, Debug, Default)]
pub struct CreateOptions {
    pub title: String,
    pub message: Option<String>,
    /// Optional running node to enrich the report from (`/version`+`/healthz`).
    pub url: Option<String>,
    /// Override the target repo (`owner/name`); defaults to `tool.repo`.
    pub repo: Option<String>,
    pub label: Vec<String>,
    pub dry_run: bool,
    pub yes: bool,
}

/// `issue create` — file (or preview) a structured issue.
#[cfg(feature = "online")]
pub async fn create(tool: &ToolInfo, opts: CreateOptions) -> Result<()> {
    let repo = resolve_repo(tool, opts.repo.as_deref()).to_string();
    let labels = report_labels(tool, &opts.label);
    let client = http_client(tool)?;

    let node = match opts.url.as_deref() {
        Some(url) => Some(fetch_node_status(&client, url).await),
        None => None,
    };
    let body = assemble_body(
        opts.message.as_deref(),
        &render_diagnostics(tool, node.as_deref()),
    );

    if opts.dry_run {
        print_preview(&repo, &opts.title, &body, &labels);
        return Ok(());
    }

    if let Some(courier_url) = crate::resolve_courier_url() {
        if !opts.yes && !crate::confirm(&format!("file this issue to {repo}?"))? {
            println!("aborted");
            println!("next: done");
            return Ok(());
        }
        let (owner, name) = split_repo_owner_name(&repo)?;
        let url = courier_create_url(&courier_url, owner, name);
        let created =
            submit_issue_via_courier(&client, &url, &issue_payload(&opts.title, &body, &labels))
                .await?;
        print_created_issue(&created, &labels);
        return Ok(());
    }

    match crate::resolve_github_token() {
        Some(token) => {
            if !opts.yes && !crate::confirm(&format!("file this issue to {repo}?"))? {
                println!("aborted");
                println!("next: done");
                return Ok(());
            }
            let created = submit_issue(
                &client,
                &repo,
                &token,
                &issue_payload(&opts.title, &body, &labels),
            )
            .await?;
            print_created_issue(&created, &labels);
        }
        None => {
            note_no_credential();
            print_fallback(&repo, &opts.title, &body, &labels);
        }
    }
    Ok(())
}

/// Offline build: assemble + print (`--dry-run`) or the browser fallback.
#[cfg(not(feature = "online"))]
pub async fn create(tool: &ToolInfo, opts: CreateOptions) -> Result<()> {
    let repo = resolve_repo(tool, opts.repo.as_deref()).to_string();
    let labels = report_labels(tool, &opts.label);
    let body = assemble_body(opts.message.as_deref(), &render_diagnostics(tool, None));
    if opts.dry_run {
        print_preview(&repo, &opts.title, &body, &labels);
    } else {
        note_offline_build();
        print_fallback(&repo, &opts.title, &body, &labels);
    }
    Ok(())
}
