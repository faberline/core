use anyhow::{Context, Result};
use rmcp::ServiceExt;

use crate::application::McpApplication;

/// Serve a product handler over MCP standard input and output.
pub async fn serve_stdio<A: McpApplication>(application: A) -> Result<()> {
    let service = application
        .serve(rmcp::transport::stdio())
        .await
        .context("start MCP stdio transport")?;
    service
        .waiting()
        .await
        .context("wait for MCP stdio transport")?;
    Ok(())
}
