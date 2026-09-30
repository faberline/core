//! `upgrade`, wired to the HTTP client, the terminal and the running binary.

use anyhow::Result;

use crate::application::upgrade::{self as use_case, Options};
use crate::infrastructure::confirm::TerminalPrompt;
use crate::infrastructure::http::HttpClient;
use crate::infrastructure::self_install::SelfReplace;
use crate::ToolInfo;

/// Run `<tool> upgrade`. Offline builds (no `online` feature) only support the
/// install path through a clear error; `--check` still degrades clearly.
pub async fn run(tool: &ToolInfo, opts: Options) -> Result<()> {
    use_case::run(tool, opts, &TerminalPrompt, &SelfReplace, || {
        HttpClient::new(format!("{}-upgrade/{}", tool.project, tool.version))
    })
    .await
}
