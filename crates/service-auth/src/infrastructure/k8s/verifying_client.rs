use std::net::SocketAddr;

/// A client that connects to `addr` while addressing — and verifying —
/// `server_name`, against `ca_pem` and nothing else.
///
/// This is what makes a forwarded local socket safe to use as a transport. A
/// port-forward's local end is `127.0.0.1`, but the thing on the far end of it
/// is a named Service holding a certificate for that name; a client that
/// verified `127.0.0.1` would be verifying the tunnel rather than what the
/// tunnel reaches, and the only certificate that could satisfy it is one
/// nobody should issue. So the URL carries the real name — SNI, hostname
/// verification, and any `Host` header all follow from it — and only address
/// resolution is redirected.
///
/// The built-in root store is switched off deliberately. A private CA that is
/// merely *added* to the public roots means any public CA can still vouch for
/// this name, which is the whole property a private trust domain buys.
pub fn verifying_client(
    ca_pem: &str,
    server_name: &str,
    addr: SocketAddr,
) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .tls_built_in_root_certs(false)
        .add_root_certificate(reqwest::Certificate::from_pem(ca_pem.as_bytes())?)
        .resolve(server_name, addr)
        .build()
}
