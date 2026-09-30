//! The issue verbs, wired to the HTTP client, the process environment and
//! the terminal.

use anyhow::Result;

use crate::application::issue::comment::CommentOptions;
use crate::application::issue::create::CreateOptions;
use crate::application::issue::search::SearchOptions;
use crate::application::issue::{comment as comment_case, create as create_case};
use crate::application::issue::{search as search_case, view as view_case};
use crate::domain::remote::RemoteError;
use crate::infrastructure::confirm::TerminalPrompt;
use crate::infrastructure::http::HttpClient;
use crate::infrastructure::issue::EnvTracker;
use crate::ToolInfo;

/// `issue create` — file (or preview) a structured issue.
pub async fn create(tool: &ToolInfo, opts: CreateOptions) -> Result<()> {
    create_case::create(tool, opts, &EnvTracker, &TerminalPrompt, || client(tool)).await
}

/// `issue comment` — ensure an issue is open and attach a verification-failed note.
pub async fn comment(tool: &ToolInfo, opts: CommentOptions) -> Result<()> {
    comment_case::comment(tool, opts, &EnvTracker, &TerminalPrompt, || client(tool)).await
}

/// `issue search` — list/search this tool's issues (filtered to `app:<name>`).
pub async fn search(tool: &ToolInfo, opts: SearchOptions) -> Result<()> {
    search_case::search(tool, opts, &EnvTracker, || client(tool)).await
}

/// `issue view` — print a single issue by number.
pub async fn view(tool: &ToolInfo, number: u64) -> Result<()> {
    view_case::view(tool, number, &EnvTracker, || client(tool)).await
}

/// The client every issue verb sends with: user agent `<project>-issue/<version>`.
fn client(tool: &ToolInfo) -> Result<HttpClient, RemoteError> {
    HttpClient::new(format!("{}-issue/{}", tool.project(), tool.version()))
}
