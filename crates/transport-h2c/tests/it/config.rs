use std::time::Duration;

use transport_h2c::ManagerConfig;

#[cfg(feature = "server")]
const EIGHT_STREAMS: transport_h2c::ConnectionOptions = transport_h2c::ConnectionOptions::new(8);

#[cfg(feature = "server")]
#[test]
fn connection_options_new_and_default_set_the_stream_cap() {
    assert_eq!(EIGHT_STREAMS.max_concurrent_streams(), 8);
    assert_eq!(
        transport_h2c::ConnectionOptions::default().max_concurrent_streams(),
        4096
    );
}

#[test]
fn manager_config_builders_set_every_field() {
    let config = ManagerConfig::default()
        .with_min_connections(2)
        .with_max_connections(9)
        .with_max_keepalive_connections(3)
        .with_max_in_flight_per_origin(40)
        .with_grow_threshold(7)
        .with_pool_timeout(Duration::from_millis(11))
        .with_connect_timeout(Duration::from_millis(12))
        .with_request_timeout(None)
        .with_ping_interval(Duration::from_millis(13))
        .with_idle_timeout(Duration::from_millis(14))
        .with_stream_window(1 << 16)
        .with_conn_window(1 << 17)
        .with_max_frame(1 << 15);
    assert_eq!(config.min_connections(), 2);
    assert_eq!(config.max_connections(), 9);
    assert_eq!(config.max_keepalive_connections(), 3);
    assert_eq!(config.max_in_flight_per_origin(), 40);
    assert_eq!(config.grow_threshold(), 7);
    assert_eq!(config.pool_timeout(), Duration::from_millis(11));
    assert_eq!(config.connect_timeout(), Duration::from_millis(12));
    assert_eq!(config.request_timeout(), None);
    assert_eq!(config.ping_interval(), Duration::from_millis(13));
    assert_eq!(config.idle_timeout(), Duration::from_millis(14));
    assert_eq!(config.stream_window(), 1 << 16);
    assert_eq!(config.conn_window(), 1 << 17);
    assert_eq!(config.max_frame(), 1 << 15);
}

#[test]
fn for_concurrency_caps_admission_at_the_target() {
    let config = ManagerConfig::for_concurrency(64);
    assert_eq!(config.max_in_flight_per_origin(), 64);
    assert!(config.max_connections() >= config.min_connections());
    assert_eq!(
        ManagerConfig::default().request_timeout(),
        Some(Duration::from_secs(30))
    );
}
