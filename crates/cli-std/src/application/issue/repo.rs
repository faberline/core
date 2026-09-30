#[cfg(feature = "online")]
use anyhow::Result;

use crate::ToolInfo;

/// The repo to file against: `--repo` else the tool's default.
pub fn resolve_repo<'a>(tool: &'a ToolInfo, repo: Option<&'a str>) -> &'a str {
    repo.unwrap_or(tool.repo())
}

/// Split `"owner/name"` into its two path segments for courier's
/// `/v1/issues/{owner}/{name}...` routes.
#[cfg(feature = "online")]
pub(super) fn split_repo_owner_name(repo: &str) -> Result<(&str, &str)> {
    repo.split_once('/')
        .filter(|(owner, name)| !owner.is_empty() && !name.is_empty())
        .ok_or_else(|| anyhow::anyhow!("repo must be \"owner/name\", got {repo:?}"))
}
