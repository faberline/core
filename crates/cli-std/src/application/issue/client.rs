use anyhow::Result;

use crate::ToolInfo;

#[cfg(feature = "online")]
pub(super) fn http_client(tool: &ToolInfo) -> Result<reqwest::Client> {
    use anyhow::Context;
    reqwest::Client::builder()
        .user_agent(format!("{}-issue/{}", tool.project, tool.version))
        .build()
        .context("build HTTP client")
}
