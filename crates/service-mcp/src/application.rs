use rmcp::ServerHandler;

/// A product MCP handler that can receive a caller credential at the HTTP
/// transport boundary. Tool schemas and handlers remain in the product.
pub trait McpApplication: ServerHandler + Clone + Send + Sync + 'static {
    fn with_bearer_token(&self, token: Option<String>) -> Self;
}
