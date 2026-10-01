use std::sync::Arc;

use axum::{body::Body, extract::Request, response::Response, routing::any, Router};
use tower::ServiceExt as TowerServiceExt;

use crate::application::McpApplication;
use crate::config::HttpTransportConfig;

/// Build one Streamable HTTP route with shared sessions and browser policy.
pub fn streamable_http_router<A: McpApplication>(
    path: &str,
    application: A,
    config: HttpTransportConfig,
) -> Router {
    let sessions = Arc::new(
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager::default(),
    );
    let config = config.into_rmcp();
    Router::new().route(
        path,
        any(move |request: Request| {
            let application = application.clone();
            let sessions = sessions.clone();
            let config = config.clone();
            async move { handle_http(request, application, sessions, config).await }
        }),
    )
}

async fn handle_http<A: McpApplication>(
    request: Request,
    application: A,
    sessions: Arc<rmcp::transport::streamable_http_server::session::local::LocalSessionManager>,
    config: rmcp::transport::streamable_http_server::StreamableHttpServerConfig,
) -> Response {
    let token = service_auth::bearer_token(request.headers()).map(str::to_owned);
    let handler = application.with_bearer_token(token);
    let service = rmcp::transport::streamable_http_server::StreamableHttpService::new(
        move || Ok(handler.clone()),
        sessions,
        config,
    );
    let response = match service.oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    };
    let (parts, body) = response.into_parts();
    Response::from_parts(parts, Body::new(body))
}
