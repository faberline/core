use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::header::{HeaderMap, HeaderName, AUTHORIZATION};
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use tokio::sync::{mpsc, oneshot};

use crate::application::k8s::{TokenRequestError, TokenSource};

/// The largest request or response body this will buffer.
///
/// The proxy reads each body whole rather than streaming it, which is the
/// simplification a short-lived local process can afford and a server cannot.
/// The cap exists so that "afford" stays true.
const MAX_BODY_BYTES: usize = 512 * 1024 * 1024;

/// Headers that describe the hop rather than the message, and so must not be
/// copied onto the next one. `authorization` is in this list because the proxy
/// supplies its own, and `host` because the upstream URL determines it.
const PER_HOP_HEADERS: &[&str] = &[
    "authorization",
    "connection",
    "content-length",
    "host",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

fn is_per_hop(name: &HeaderName) -> bool {
    PER_HOP_HEADERS.contains(&name.as_str())
}

#[derive(Clone)]
struct ProxyState {
    upstream: String,
    tokens: Arc<TokenSource>,
    client: reqwest::Client,
    fatal: mpsc::Sender<TokenRequestError>,
}

/// A running loopback listener that authenticates what passes through it.
///
/// Dropping this stops the listener: the shutdown signal is held here, and
/// `axum`'s graceful shutdown fires when it goes. So a caller that returns
/// early — including by `?` on an unrelated error — does not leave a
/// credential-bearing port open behind it.
pub struct LoopbackProxy {
    addr: SocketAddr,
    fatal: mpsc::Receiver<TokenRequestError>,
    // Held, never sent on: the receiver inside the serving task completes when
    // this is dropped, which is the shutdown edge.
    _shutdown: oneshot::Sender<()>,
    handle: tokio::task::JoinHandle<()>,
}

impl LoopbackProxy {
    /// Bind an ephemeral loopback port and start forwarding to `upstream`.
    ///
    /// Returns once the port is bound and accepting, so a caller may hand the
    /// URL to a child immediately without racing it.
    pub async fn start(
        upstream: impl Into<String>,
        tokens: Arc<TokenSource>,
    ) -> std::io::Result<Self> {
        Self::start_with_client(upstream, tokens, reqwest::Client::new()).await
    }

    /// The same, forwarding through a caller-supplied client.
    ///
    /// The client is the seam because "how the upstream is trusted" is not this
    /// module's question. A private-CA deployment builds one with
    /// [`verifying_client`]; a plaintext development deployment passes the
    /// default. Either way what happens to the credential is identical, which
    /// is why the two share this code path rather than forking it.
    pub async fn start_with_client(
        upstream: impl Into<String>,
        tokens: Arc<TokenSource>,
        client: reqwest::Client,
    ) -> std::io::Result<Self> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;
        let (fatal_tx, fatal_rx) = mpsc::channel(1);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let state = ProxyState {
            upstream: upstream.into().trim_end_matches('/').to_string(),
            tokens,
            client,
            fatal: fatal_tx,
        };
        let app = axum::Router::new().fallback(forward).with_state(state);

        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        Ok(Self {
            addr,
            fatal: fatal_rx,
            _shutdown: shutdown_tx,
            handle,
        })
    }

    /// The address the child is given. Always on `127.0.0.1`.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The URL the child is given — and, by design, the only thing it is
    /// given.
    pub fn local_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Resolves when the proxy can no longer authenticate a request.
    ///
    /// A caller should select on this alongside whatever else it is waiting
    /// for, and shut down when it fires: past this point every forwarded
    /// request is a `503`, and the useful thing to do is say why once rather
    /// than let the child discover it request by request.
    pub async fn next_fatal(&mut self) -> Option<TokenRequestError> {
        self.fatal.recv().await
    }

    /// Stop the listener and wait for in-flight requests to finish.
    pub async fn shutdown(self) {
        let Self {
            _shutdown, handle, ..
        } = self;
        drop(_shutdown);
        let _ = handle.await;
    }
}

async fn forward(State(state): State<ProxyState>, request: Request<Body>) -> Response {
    let token = match state.tokens.token().await {
        Ok(token) => token,
        Err(error) => {
            let message = error.to_string();
            // Best-effort: the channel holds one, and one report is the point.
            let _ = state.fatal.try_send(error);
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("this connection can no longer be authenticated: {message}\n"),
            )
                .into_response();
        }
    };

    let (parts, body) = request.into_parts();
    let body = match axum::body::to_bytes(body, MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("request body exceeds the {MAX_BODY_BYTES}-byte forwarding limit\n"),
            )
                .into_response()
        }
    };

    let target = format!(
        "{}{}",
        state.upstream,
        parts
            .uri
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/")
    );

    let mut outbound = state.client.request(parts.method, &target);
    for (name, value) in parts.headers.iter() {
        if is_per_hop(name) {
            continue;
        }
        outbound = outbound.header(name.clone(), value.clone());
    }
    // Last, and unconditional: whatever the child sent under this name has
    // already been dropped above.
    outbound = outbound.header(AUTHORIZATION, format!("Bearer {}", token.expose()));

    let upstream = match outbound.body(body).send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                // `error` renders the URL and the transport failure. It cannot
                // render the header that was set, and this is the only place
                // the token and an error message are in scope together.
                format!("upstream request failed: {error}\n"),
            )
                .into_response();
        }
    };

    let status = upstream.status();
    let headers = upstream.headers().clone();
    let payload = match upstream.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("upstream response could not be read: {error}\n"),
            )
                .into_response()
        }
    };

    let mut response_headers = HeaderMap::new();
    for (name, value) in headers.iter() {
        if is_per_hop(name) {
            continue;
        }
        response_headers.insert(name.clone(), value.clone());
    }
    (status, response_headers, payload).into_response()
}

#[cfg(test)]
mod tests;
