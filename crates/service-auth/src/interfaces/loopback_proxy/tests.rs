use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;

use crate::k8s::cache::{Clock, ManualClock};
use crate::k8s::token_request::{MintedToken, TokenMinter, TokenRequestTarget};

const CANARY: &str = "canary-proxy-token-must-never-reach-the-child";

struct Minter {
    clock: Arc<ManualClock>,
    calls: AtomicU64,
    fail_after: u64,
}

#[async_trait]
impl TokenMinter for Minter {
    async fn mint(&self, target: &TokenRequestTarget) -> Result<MintedToken, TokenRequestError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n >= self.fail_after {
            return Err(TokenRequestError::Forbidden {
                username: Some("alice@example.com".to_string()),
                namespace: target.namespace().to_string(),
                service_account: target.service_account().to_string(),
                detail: "the grant was revoked".to_string(),
            });
        }
        let now = self.clock.now_millis();
        Ok(MintedToken::new(
            format!("{CANARY}-{n}"),
            now,
            now + 600_000,
        ))
    }
}

fn source(clock: Arc<ManualClock>, fail_after: u64) -> Arc<TokenSource> {
    let minter = Arc::new(Minter {
        clock: clock.clone(),
        calls: AtomicU64::new(0),
        fail_after,
    });
    Arc::new(TokenSource::with_clock(
        minter,
        TokenRequestTarget::new("ops", "app-client", "callee.example.com").expect("valid target"),
        clock,
    ))
}

/// Everything the upstream saw, so an assertion can be about the request
/// that arrived rather than about the proxy's internal state.
#[derive(Default)]
struct Seen {
    authorization: Vec<String>,
    methods: Vec<String>,
    paths: Vec<String>,
    bodies: Vec<String>,
    custom: Vec<Option<String>>,
}

/// A minimal upstream. Deliberately not a mock library: what is being
/// tested is the bytes on the wire.
async fn upstream(seen: Arc<Mutex<Seen>>) -> String {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind upstream");
    let addr = listener.local_addr().expect("upstream addr");
    let app = axum::Router::new().fallback(move |request: Request<Body>| {
        let seen = seen.clone();
        async move {
            let (parts, body) = request.into_parts();
            let body = axum::body::to_bytes(body, MAX_BODY_BYTES)
                .await
                .expect("read body");
            let mut seen = seen.lock().expect("record");
            seen.authorization.push(
                parts
                    .headers
                    .get(AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("<absent>")
                    .to_string(),
            );
            seen.methods.push(parts.method.to_string());
            seen.paths.push(
                parts
                    .uri
                    .path_and_query()
                    .map(|p| p.to_string())
                    .unwrap_or_default(),
            );
            seen.bodies.push(String::from_utf8_lossy(&body).to_string());
            seen.custom.push(
                parts
                    .headers
                    .get("x-caller")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string),
            );
            (StatusCode::OK, [("x-upstream", "yes")], "{\"ok\":true}")
        }
    });
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

/// AC4: the child's request reaches the upstream carrying the minted
/// token, and the child never had to hold it.
#[tokio::test]
async fn the_proxy_attaches_the_token_and_forwards_everything_else_unchanged() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let base = upstream(seen.clone()).await;
    let clock = Arc::new(ManualClock::new(0));
    let proxy = LoopbackProxy::start(base, source(clock, u64::MAX))
        .await
        .expect("start proxy");

    assert!(
        proxy.local_url().starts_with("http://127.0.0.1:"),
        "the child must be given a loopback URL: {}",
        proxy.local_url()
    );

    let response = reqwest::Client::new()
        .post(format!(
            "{}/collections/docs/search?limit=5",
            proxy.local_url()
        ))
        .header("x-caller", "the-child")
        .body("{\"query\":{}}")
        .send()
        .await
        .expect("through the proxy");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-upstream")
            .map(|v| v.to_str().unwrap()),
        Some("yes"),
        "the upstream's own headers come back"
    );
    assert_eq!(response.text().await.expect("body"), "{\"ok\":true}");

    let seen = seen.lock().expect("read records");
    assert_eq!(seen.authorization, vec![format!("Bearer {CANARY}-0")]);
    assert_eq!(seen.methods, vec!["POST"]);
    assert_eq!(seen.paths, vec!["/collections/docs/search?limit=5"]);
    assert_eq!(seen.bodies, vec!["{\"query\":{}}"]);
    assert_eq!(
        seen.custom,
        vec![Some("the-child".to_string())],
        "headers that are not the proxy's business pass through"
    );
}

/// The child does not get to choose who it is. A request that arrives with
/// its own `Authorization` is forwarded with the proxy's, not the child's,
/// and not both.
#[tokio::test]
async fn a_child_supplied_authorization_header_is_replaced_rather_than_merged() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let base = upstream(seen.clone()).await;
    let clock = Arc::new(ManualClock::new(0));
    let proxy = LoopbackProxy::start(base, source(clock, u64::MAX))
        .await
        .expect("start proxy");

    reqwest::Client::new()
        .get(proxy.local_url())
        .header(AUTHORIZATION, "Bearer a-google-access-token")
        .send()
        .await
        .expect("through the proxy");

    let seen = seen.lock().expect("read records");
    assert_eq!(seen.authorization, vec![format!("Bearer {CANARY}-0")]);
    assert!(
        !seen.authorization[0].contains("google"),
        "the child's own credential must not reach the upstream: {:?}",
        seen.authorization
    );
}

/// AC5 through the proxy: once the grant is gone the proxy refuses, says
/// why, and tells the caller to shut down — rather than serving the token
/// it still holds until that one expires too.
#[tokio::test]
async fn a_refresh_failure_becomes_a_refusal_and_a_shutdown_signal() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let base = upstream(seen.clone()).await;
    let clock = Arc::new(ManualClock::new(0));
    let mut proxy = LoopbackProxy::start(base, source(clock.clone(), 1))
        .await
        .expect("start proxy");
    let client = reqwest::Client::new();

    let first = client
        .get(proxy.local_url())
        .send()
        .await
        .expect("the first request works");
    assert_eq!(first.status(), StatusCode::OK);

    // Past the refresh point, with the token still nominally valid.
    clock.advance(Duration::from_secs(500));

    let refused = client
        .get(proxy.local_url())
        .send()
        .await
        .expect("the proxy answers rather than hanging");
    assert_eq!(refused.status(), StatusCode::SERVICE_UNAVAILABLE);
    let explanation = refused.text().await.expect("body");
    assert!(explanation.contains("alice@example.com"), "{explanation}");
    assert!(!explanation.contains(CANARY), "{explanation}");

    let fatal = tokio::time::timeout(Duration::from_secs(5), proxy.next_fatal())
        .await
        .expect("the caller is told, rather than left polling")
        .expect("a fatal error");
    assert!(
        matches!(fatal, TokenRequestError::Forbidden { .. }),
        "{fatal:?}"
    );

    let seen = seen.lock().expect("read records");
    assert_eq!(
        seen.authorization.len(),
        1,
        "the refused request must not reach the upstream at all"
    );
}

/// AC6 at this layer: the token appears in exactly one place — the
/// `Authorization` header of the forwarded request — and in nothing the
/// child can read back.
#[tokio::test]
async fn no_response_the_child_can_read_contains_the_token() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let base = upstream(seen.clone()).await;
    let clock = Arc::new(ManualClock::new(0));
    let proxy = LoopbackProxy::start(base, source(clock, u64::MAX))
        .await
        .expect("start proxy");
    let client = reqwest::Client::new();

    let ok = client.get(proxy.local_url()).send().await.expect("ok");
    let ok_headers = format!("{:?}", ok.headers());
    let ok_body = ok.text().await.expect("body");
    assert!(!ok_headers.contains(CANARY), "{ok_headers}");
    assert!(!ok_body.contains(CANARY), "{ok_body}");

    // The bad-gateway path, where an error string and the token are in
    // scope in the same function.
    let broken = LoopbackProxy::start(
        "http://127.0.0.1:1",
        source(Arc::new(ManualClock::new(0)), u64::MAX),
    )
    .await
    .expect("start proxy");
    let failed = client
        .get(broken.local_url())
        .send()
        .await
        .expect("the proxy answers");
    assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);
    let body = failed.text().await.expect("body");
    assert!(
        !body.contains(CANARY),
        "an error echoed the credential: {body}"
    );
}

/// The listener is bound to loopback, not to every interface. Asserted on
/// the bound address rather than by attempting an off-host connection,
/// which a test cannot do portably.
#[tokio::test]
async fn the_listener_is_loopback_only_and_on_an_unpredictable_port() {
    let clock = Arc::new(ManualClock::new(0));
    let first = LoopbackProxy::start("http://127.0.0.1:1", source(clock.clone(), u64::MAX))
        .await
        .expect("start");
    let second = LoopbackProxy::start("http://127.0.0.1:1", source(clock, u64::MAX))
        .await
        .expect("start");

    assert!(first.addr().ip().is_loopback());
    assert!(second.addr().ip().is_loopback());
    assert_ne!(first.addr().port(), 0);
    assert_ne!(
        first.addr().port(),
        second.addr().port(),
        "an ephemeral port is chosen per run, not a fixed one"
    );
}

/// R7: when the proxy goes, the port goes. A caller that returns early
/// must not leave an authenticating listener behind it.
#[tokio::test]
async fn dropping_the_proxy_closes_the_port() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let base = upstream(seen).await;
    let clock = Arc::new(ManualClock::new(0));
    let proxy = LoopbackProxy::start(base, source(clock, u64::MAX))
        .await
        .expect("start");
    let url = proxy.local_url();
    let addr = proxy.addr();

    assert!(reqwest::Client::new().get(&url).send().await.is_ok());
    proxy.shutdown().await;

    let reconnect =
        tokio::time::timeout(Duration::from_secs(5), tokio::net::TcpStream::connect(addr))
            .await
            .expect("the connect attempt itself must not hang");
    assert!(
        reconnect.is_err(),
        "the loopback port is still accepting after shutdown"
    );
}
