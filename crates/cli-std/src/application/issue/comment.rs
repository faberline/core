use anyhow::Result;

use super::diagnostics::followup_comment_body;
use super::output::{print_comment_fallback, print_comment_preview};
use super::repo::resolve_repo;
use crate::domain::issue::number::{check_issue_number, IssueNumberError};
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
///
/// ```
/// let opts = cli_std::issue::CommentOptions::try_new(42)?.with_dry_run(true);
/// assert_eq!(opts.number(), 42);
/// assert_eq!(
///     cli_std::issue::CommentOptions::try_new(0).unwrap_err().to_string(),
///     "issue number must be positive"
/// );
/// # Ok::<(), cli_std::IssueNumberError>(())
/// ```
#[derive(Clone, Debug)]
pub struct CommentOptions {
    number: u64,
    message: Option<String>,
    repo: Option<String>,
    dry_run: bool,
    yes: bool,
}

impl CommentOptions {
    /// Flags for a comment on issue `number`, with every other flag unset.
    /// Rejects 0, which names no issue.
    pub fn try_new(number: u64) -> std::result::Result<Self, IssueNumberError> {
        Ok(Self {
            number: check_issue_number(number)?,
            message: None,
            repo: None,
            dry_run: false,
            yes: false,
        })
    }

    /// Sets the operator/user verification note. Without one, a standard
    /// "verification failed after closure" note is used.
    pub fn with_message(mut self, message: impl Into<Option<String>>) -> Self {
        self.message = message.into();
        self
    }

    /// Overrides the target repo (`owner/name`); it defaults to `tool.repo()`.
    pub fn with_repo(mut self, repo: impl Into<Option<String>>) -> Self {
        self.repo = repo.into();
        self
    }

    /// Prints the comment request without changing GitHub state.
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Skips the confirmation prompt.
    pub fn with_yes(mut self, yes: bool) -> Self {
        self.yes = yes;
        self
    }

    /// The issue number to comment on; never 0.
    pub fn number(&self) -> u64 {
        self.number
    }

    /// The verification note, if any.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The target repo override, if any.
    pub fn repo(&self) -> Option<&str> {
        self.repo.as_deref()
    }

    /// Whether the comment is only printed.
    pub fn dry_run(&self) -> bool {
        self.dry_run
    }

    /// Whether the confirmation prompt is skipped.
    pub fn yes(&self) -> bool {
        self.yes
    }
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
    let repo = resolve_repo(tool, opts.repo()).to_string();
    let body = followup_comment_body(tool, opts.message());

    if opts.dry_run() {
        print_comment_preview(&repo, opts.number(), &body);
        return Ok(());
    }

    if let Some(courier_url) = access.courier_url() {
        if !opts.yes()
            && !prompt.confirm(&format!(
                "comment on issue #{} in {repo} and ensure it is open?",
                opts.number()
            ))?
        {
            println!("aborted");
            println!("next: done");
            return Ok(());
        }
        let api = open()?;
        let (owner, name) = split_repo_owner_name(&repo)?;
        let url = courier_comment_url(&courier_url, owner, name, opts.number());
        let comment_url = api.post_issue_comment_via_courier(&url, &body).await?;
        println!("issue: {}", issue_url(&repo, opts.number()));
        println!("commented: {comment_url}");
        println!("next: done");
        return Ok(());
    }

    let Some(token) = access.github_token() else {
        note_no_credential_comment();
        print_comment_fallback(&repo, opts.number(), &body);
        return Ok(());
    };

    if !opts.yes()
        && !prompt.confirm(&format!(
            "comment on issue #{} in {repo} and ensure it is open?",
            opts.number()
        ))?
    {
        println!("aborted");
        println!("next: done");
        return Ok(());
    }

    let api = open()?;
    let url = api.reopen_issue(&repo, opts.number(), &token).await?;
    println!("issue: {url}");
    let comment_url = api
        .post_issue_comment(&repo, opts.number(), &token, &body)
        .await?;
    println!("commented: {comment_url}");
    println!("next: done");
    Ok(())
}

/// Offline build: print the issue URL and the comment to paste after reopening.
#[cfg(not(feature = "online"))]
pub async fn comment(tool: &ToolInfo, opts: CommentOptions) -> Result<()> {
    let repo = resolve_repo(tool, opts.repo()).to_string();
    let body = followup_comment_body(tool, opts.message());
    if opts.dry_run() {
        print_comment_preview(&repo, opts.number(), &body);
    } else {
        note_offline_comment_build();
        print_comment_fallback(&repo, opts.number(), &body);
    }
    Ok(())
}
