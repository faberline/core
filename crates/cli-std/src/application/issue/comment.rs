use anyhow::Result;

use super::diagnostics::followup_comment_body;
use super::output::{print_comment_fallback, print_comment_preview};
use super::repo::resolve_repo;
use crate::ToolInfo;

#[cfg(not(feature = "online"))]
use super::output::note_offline_comment_build;
#[cfg(feature = "online")]
use super::{output::note_no_credential_comment, repo::split_repo_owner_name};
#[cfg(feature = "online")]
use crate::domain::{
    issue::{
        tracker::{CourierApi, GitHubApi, TrackerAccess},
        url::{courier_comment_url, issue_url},
    },
    prompt::Confirm,
    remote::RemoteError,
};

/// Flags for `issue comment`.
#[derive(Clone, Debug, Default)]
pub struct CommentOptions {
    /// Issue number to comment on.
    pub number: u64,
    /// Optional operator/user verification note. When empty, a standard
    /// "verification failed after closure" note is used.
    pub message: Option<String>,
    /// Override the target repo (`owner/name`); defaults to `tool.repo()`.
    pub repo: Option<String>,
    /// Print the comment request without changing GitHub state.
    pub dry_run: bool,
    /// Skip the confirmation prompt.
    pub yes: bool,
}

fn validate_issue_number(number: u64) -> Result<()> {
    if number == 0 {
        anyhow::bail!("issue number must be positive");
    }
    Ok(())
}

/// `issue comment` — ensure an issue is open and attach a verification-failed
/// note. `open` builds the HTTP client once the comment is confirmed; `access`
/// says whether to go through courier and which GitHub token to use.
#[cfg(feature = "online")]
pub(crate) async fn comment<A>(
    tool: &ToolInfo,
    opts: CommentOptions,
    access: &impl TrackerAccess,
    prompt: &impl Confirm,
    open: impl FnOnce() -> Result<A, RemoteError>,
) -> Result<()>
where
    A: GitHubApi + CourierApi,
{
    validate_issue_number(opts.number)?;
    let repo = resolve_repo(tool, opts.repo.as_deref()).to_string();
    let body = followup_comment_body(tool, opts.message.as_deref());

    if opts.dry_run {
        print_comment_preview(&repo, opts.number, &body);
        return Ok(());
    }

    if let Some(courier_url) = access.courier_url() {
        if !opts.yes
            && !prompt.confirm(&format!(
                "comment on issue #{} in {repo} and ensure it is open?",
                opts.number
            ))?
        {
            println!("aborted");
            println!("next: done");
            return Ok(());
        }
        let api = open()?;
        let (owner, name) = split_repo_owner_name(&repo)?;
        let url = courier_comment_url(&courier_url, owner, name, opts.number);
        let comment_url = api.post_issue_comment_via_courier(&url, &body).await?;
        println!("issue: {}", issue_url(&repo, opts.number));
        println!("commented: {comment_url}");
        println!("next: done");
        return Ok(());
    }

    let Some(token) = access.github_token() else {
        note_no_credential_comment();
        print_comment_fallback(&repo, opts.number, &body);
        return Ok(());
    };

    if !opts.yes
        && !prompt.confirm(&format!(
            "comment on issue #{} in {repo} and ensure it is open?",
            opts.number
        ))?
    {
        println!("aborted");
        println!("next: done");
        return Ok(());
    }

    let api = open()?;
    let url = api.reopen_issue(&repo, opts.number, &token).await?;
    println!("issue: {url}");
    let comment_url = api
        .post_issue_comment(&repo, opts.number, &token, &body)
        .await?;
    println!("commented: {comment_url}");
    println!("next: done");
    Ok(())
}

/// Offline build: print the issue URL and the comment to paste after reopening.
#[cfg(not(feature = "online"))]
pub async fn comment(tool: &ToolInfo, opts: CommentOptions) -> Result<()> {
    validate_issue_number(opts.number)?;
    let repo = resolve_repo(tool, opts.repo.as_deref()).to_string();
    let body = followup_comment_body(tool, opts.message.as_deref());
    if opts.dry_run {
        print_comment_preview(&repo, opts.number, &body);
    } else {
        note_offline_comment_build();
        print_comment_fallback(&repo, opts.number, &body);
    }
    Ok(())
}
