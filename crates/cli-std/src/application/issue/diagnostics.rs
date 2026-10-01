use crate::domain::issue::body::assemble_body;
use crate::ToolInfo;

/// Render the diagnostics block from the tool identity (+ optional node line).
pub fn render_diagnostics(tool: &ToolInfo, node: Option<&str>) -> String {
    let mut s = String::from("## Diagnostics\n");
    s.push_str(&format!("- {} version: {}\n", tool.project, tool.version));
    s.push_str(&format!("- target: {}\n", tool.target));
    s.push_str(&format!("- git sha: {}\n", tool.git_sha));
    s.push_str(&format!("- built at: {}\n", tool.built_at));
    s.push_str(&format!(
        "- os/arch: {}/{}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    if let Some(node) = node {
        s.push_str(&format!("- node: {node}\n"));
    }
    s
}

/// Assemble the follow-up comment used by `issue comment`.
pub fn followup_comment_body(tool: &ToolInfo, message: Option<&str>) -> String {
    let message = message
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or("User-side verification failed after closure; reopening for follow-up.");
    assemble_body(Some(message), &render_diagnostics(tool, None))
}
