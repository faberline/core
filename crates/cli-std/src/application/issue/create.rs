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
///
/// ```
/// let opts = cli_std::issue::CreateOptions::new("lumen: crash on start")
///     .with_message(Some("seen after upgrade".to_string()))
///     .with_dry_run(true);
/// assert_eq!(opts.title(), "lumen: crash on start");
/// assert!(opts.url().is_none());
/// ```
#[derive(Clone, Debug, Default)]
pub struct CreateOptions {
    title: String,
    message: Option<String>,
    url: Option<String>,
    repo: Option<String>,
    label: Vec<String>,
    dry_run: bool,
    yes: bool,
}

impl CreateOptions {
    /// Flags for an issue titled `title`, with every other flag unset.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }

    /// Sets the operator's description, placed above the diagnostics block.
    pub fn with_message(mut self, message: impl Into<Option<String>>) -> Self {
        self.message = message.into();
        self
    }

    /// Sets a running node to enrich the report from (`/version`+`/healthz`).
    pub fn with_url(mut self, url: impl Into<Option<String>>) -> Self {
        self.url = url.into();
        self
    }

    /// Overrides the target repo (`owner/name`); it defaults to `tool.repo()`.
    pub fn with_repo(mut self, repo: impl Into<Option<String>>) -> Self {
        self.repo = repo.into();
        self
    }

    /// Sets the caller's labels; `app:<project>` and `type:report` are always
    /// added to them.
    pub fn with_label(mut self, label: Vec<String>) -> Self {
        self.label = label;
        self
    }

    /// Prints the issue instead of filing it.
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Skips the confirmation prompt.
    pub fn with_yes(mut self, yes: bool) -> Self {
        self.yes = yes;
        self
    }

    /// The issue title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The operator's description, if any.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The running node to enrich the report from, if any.
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// The target repo override, if any.
    pub fn repo(&self) -> Option<&str> {
        self.repo.as_deref()
    }

    /// The caller's labels, before the canonical ones are added.
    pub fn label(&self) -> &[String] {
        &self.label
    }

    /// Whether the issue is only printed.
    pub fn dry_run(&self) -> bool {
        self.dry_run
    }

    /// Whether the confirmation prompt is skipped.
    pub fn yes(&self) -> bool {
        self.yes
    }
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
    let repo = resolve_repo(tool, opts.repo()).to_string();
    let labels = report_labels(tool, opts.label());
    let api = open()?;

    let node = match opts.url() {
        Some(url) => Some(api.node_status(url).await),
        None => None,
    };
    let body = assemble_body(opts.message(), &render_diagnostics(tool, node.as_deref()));

    if opts.dry_run() {
        print_preview(&repo, opts.title(), &body, &labels);
        return Ok(());
    }

    if let Some(courier_url) = access.courier_url() {
        if !opts.yes() && !prompt.confirm(&format!("file this issue to {repo}?"))? {
            println!("aborted");
            println!("next: done");
            return Ok(());
        }
        let (owner, name) = split_repo_owner_name(&repo)?;
        let url = courier_create_url(&courier_url, owner, name);
        let created = api
            .submit_issue_via_courier(&url, &issue_payload(opts.title(), &body, &labels))
            .await?;
        print_created_issue(&created, &labels);
        return Ok(());
    }

    match access.github_token() {
        Some(token) => {
            if !opts.yes() && !prompt.confirm(&format!("file this issue to {repo}?"))? {
                println!("aborted");
                println!("next: done");
                return Ok(());
            }
            let created = api
                .submit_issue(&repo, &token, &issue_payload(opts.title(), &body, &labels))
                .await?;
            print_created_issue(&created, &labels);
        }
        None => {
            note_no_credential();
            print_fallback(&repo, opts.title(), &body, &labels);
        }
    }
    Ok(())
}

/// Offline build: assemble + print (`--dry-run`) or the browser fallback.
#[cfg(not(feature = "online"))]
pub async fn create(tool: &ToolInfo, opts: CreateOptions) -> Result<()> {
    let repo = resolve_repo(tool, opts.repo()).to_string();
    let labels = report_labels(tool, opts.label());
    let body = assemble_body(opts.message(), &render_diagnostics(tool, None));
    if opts.dry_run() {
        print_preview(&repo, opts.title(), &body, &labels);
    } else {
        note_offline_build();
        print_fallback(&repo, opts.title(), &body, &labels);
    }
    Ok(())
}
