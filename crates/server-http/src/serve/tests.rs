use std::time::Duration;

use super::*;
use axum::{routing::get, Router};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

#[tokio::test]
async fn serves_http1_and_h2c_on_one_listener_with_tunable_options() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let app = Router::new().route("/healthz", get(|| async { "ok" }));
    let options = HttpServerOptions {
        max_concurrent_streams: 17,
        drain_timeout: Duration::from_secs(1),
        ..Default::default()
    };

    let server = tokio::spawn(serve_h2c_with_options(listener, app, options, async move {
        let _ = shutdown_rx.await;
    }));

    let mut http1 = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connect HTTP/1.1");
    http1
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("write HTTP/1.1 request");
    let mut http1_response = Vec::new();
    http1
        .read_to_end(&mut http1_response)
        .await
        .expect("read HTTP/1.1 response");
    let http1_response = String::from_utf8_lossy(&http1_response);
    assert!(
        http1_response.starts_with("HTTP/1.1 200"),
        "plain HTTP/1.1 must share the listener: {http1_response}"
    );
    assert!(
        http1_response.ends_with("ok"),
        "plain HTTP/1.1 must dispatch the router body: {http1_response}"
    );

    let client = transport_h2c::h2c_client().expect("h2c client");
    let response = tokio::time::timeout(
        Duration::from_secs(3),
        client.get(format!("http://{addr}/healthz")).send(),
    )
    .await
    .expect("h2c response timeout")
    .expect("h2c response");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(response.text().await.expect("body"), "ok");
    drop(client);

    let _ = shutdown_tx.send(());
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .expect("server shutdown")
        .expect("server task");
}
