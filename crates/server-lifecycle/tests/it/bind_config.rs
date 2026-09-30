use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use server_lifecycle::BindConfig;

#[test]
fn bind_config_round_trips_a_socket_addr() {
    const LOOPBACK: BindConfig = BindConfig::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7373);
    assert_eq!(LOOPBACK.host(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(LOOPBACK.port(), 7373);
    let addr: SocketAddr = "127.0.0.1:7373".parse().unwrap();
    assert_eq!(BindConfig::from(addr), LOOPBACK);
    assert_eq!(LOOPBACK.socket_addr(), addr);
    assert_eq!(BindConfig::localhost(7373), LOOPBACK);
}
