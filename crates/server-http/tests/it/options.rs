use std::time::Duration;

use server_http::{HttpServerOptions, TlsListenerMetrics, TlsServerOptions};
use server_lifecycle::{ConnectionBudget, DrainController};
use server_tcp::TcpSocketOptions;

#[test]
fn http_options_keep_defaults_and_builder_settings() {
    let defaults = HttpServerOptions::default();
    assert_eq!(defaults.max_concurrent_streams(), 4096);
    assert_eq!(defaults.drain_timeout(), Duration::from_secs(5));
    assert!(defaults.connection_budget().is_none());
    assert_eq!(defaults.socket(), TcpSocketOptions::default());

    let drain = DrainController::new();
    let mut options = defaults
        .with_max_concurrent_streams(17)
        .with_drain_timeout(Duration::from_secs(1))
        .with_connection_budget(ConnectionBudget::new(2))
        .with_drain(drain.clone())
        .with_socket(TcpSocketOptions::default().with_backlog(64));
    assert_eq!(options.max_concurrent_streams(), 17);
    assert_eq!(options.drain_timeout(), Duration::from_secs(1));
    assert!(options.connection_budget().is_some());
    assert_eq!(options.socket().backlog(), 64);
    drain.start_drain();
    assert!(options.drain().is_draining());

    options.set_drain_timeout(Duration::from_millis(250));
    assert_eq!(options.drain_timeout(), Duration::from_millis(250));
}

#[test]
fn tls_options_carry_http_options_and_metrics() {
    let metrics = TlsListenerMetrics::new();
    let options = TlsServerOptions::default()
        .with_http(HttpServerOptions::default().with_max_concurrent_streams(9))
        .with_metrics(metrics.clone());
    assert_eq!(options.http().max_concurrent_streams(), 9);
    assert_eq!(options.metrics().snapshot(), metrics.snapshot());
}
