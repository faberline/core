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
    output::{note_no_credential, print_created_issue},
    repo::split_repo_owner_name,
};
#[cfg(feature = "online")]
use crate::domain::{
    issue::{
        payload::issue_payload,
        tracker::{CourierApi, GitHubApi, NodeProbe, TrackerAccess},
        url::courier_create_url,
    },
    prompt::Confirm,
    remote::RemoteError,
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

/// `issue create` — file (or preview) a structured issue. `open` builds the
/// HTTP client; `access` says whether to go through courier and which GitHub
/// token to use; `prompt` asks before filing.
#[cfg(feature = "online")]
pub(crate) async fn create<A>(
    tool: &ToolInfo,
    opts: CreateOptions,
    access: &impl TrackerAccess,
    prompt: &impl Confirm,
    open: impl FnOnce() -> Result<A, RemoteError>,
) -> Result<()>
where
    A: GitHubApi + CourierApi + NodeProbe,
{
    let repo = resolve_repo(tool, opts.repo.as_deref()).to_string();
    let labels = report_labels(tool, &opts.label);
    let api = open()?;

    let node = match opts.url.as_deref() {
        Some(url) => Some(api.node_status(url).await),
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

    if let Some(courier_url) = access.courier_url() {
        if !opts.yes && !prompt.confirm(&format!("file this issue to {repo}?"))? {
            println!("aborted");
            println!("next: done");
            return Ok(());
        }
        let (owner, name) = split_repo_owner_name(&repo)?;
        let url = courier_create_url(&courier_url, owner, name);
        let created = api
            .submit_issue_via_courier(&url, &issue_payload(&opts.title, &body, &labels))
            .await?;
        print_created_issue(&created, &labels);
        return Ok(());
    }

    match access.github_token() {
        Some(token) => {
            if !opts.yes && !prompt.confirm(&format!("file this issue to {repo}?"))? {
                println!("aborted");
                println!("next: done");
                return Ok(());
            }
            let created = api
                .submit_issue(&repo, &token, &issue_payload(&opts.title, &body, &labels))
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
