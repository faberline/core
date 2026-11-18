//! Product-neutral templates for security checks on a service's HTTP API: how
//! it answers a missing credential, a credential of the wrong role and a valid
//! credential. The base URL, the path and the credentials are parameters.

use std::time::Duration;

/// GET `path` on `base`, with `token` as a bearer credential when given.
/// Returns the full URL and the status code. Never retries.
///
/// # Panics
///
/// Panics when the HTTP client cannot be built or the request fails.
async fn get_status(base: &str, path: &str, token: Option<&str>) -> (String, u16) {
    let url = format!("{}{}", base.trim_end_matches('/'), path);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|e| panic!("http client: {e}"));
    let mut request = client.get(&url);
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let response = request
        .send()
        .await
        .unwrap_or_else(|e| panic!("security: GET {url} failed: {e}"));
    let status = response.status().as_u16();
    (url, status)
}

/// Checks that GET `path` without a credential is refused with 401.
///
/// # Panics
///
/// Panics unless the status is 401, or when the request fails.
pub async fn assert_rejects_missing_credential(base: &str, path: &str) {
    let (url, status) = get_status(base, path, None).await;
    if status != 401 {
        panic!("security: GET {url} without a credential returned {status}, expected 401");
    }
}

/// Checks that GET `path` with a valid credential of a role that must not
/// reach it is refused with 403.
///
/// # Panics
///
/// Panics unless the status is 403, or when the request fails.
pub async fn assert_rejects_wrong_role(base: &str, path: &str, token: &str) {
    let (url, status) = get_status(base, path, Some(token)).await;
    if status != 403 {
        panic!("security: GET {url} with a wrong role returned {status}, expected 403");
    }
}

/// Checks that GET `path` with a valid credential is accepted (2xx).
///
/// # Panics
///
/// Panics unless the status is 2xx, or when the request fails.
pub async fn assert_accepts_credential(base: &str, path: &str, token: &str) {
    let (url, status) = get_status(base, path, Some(token)).await;
    if !(200..300).contains(&status) {
        panic!("security: GET {url} with a valid credential returned {status}, expected 2xx");
    }
}

/// Installs the process crypto provider. A second install is ignored.
fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Connects to `addr` over TLS, trusting only the CA in `ca_pem`, with SNI
/// `server_name`. Returns the DER bytes of the certificate that the server
/// presented first, or the error text. Gives up after 5 seconds.
async fn handshake(addr: &str, server_name: &str, ca_pem: &str) -> Result<Vec<u8>, String> {
    use rustls::pki_types::{pem::PemObject, CertificateDer, ServerName};

    install_crypto_provider();
    let mut roots = rustls::RootCertStore::empty();
    for cert in CertificateDer::pem_slice_iter(ca_pem.as_bytes()) {
        let cert = cert.map_err(|e| format!("bad CA PEM: {e}"))?;
        roots.add(cert).map_err(|e| format!("bad CA PEM: {e}"))?;
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
    let name = ServerName::try_from(server_name.to_string()).map_err(|e| e.to_string())?;
    let work = async move {
        let tcp = tokio::net::TcpStream::connect(addr)
            .await
            .map_err(|e| e.to_string())?;
        let stream = connector
            .connect(name, tcp)
            .await
            .map_err(|e| e.to_string())?;
        let (_, session) = stream.get_ref();
        let first = session
            .peer_certificates()
            .and_then(|certs| certs.first())
            .ok_or_else(|| "no peer certificate".to_string())?;
        Ok::<Vec<u8>, String>(first.as_ref().to_vec())
    };
    tokio::time::timeout(Duration::from_secs(5), work)
        .await
        .unwrap_or_else(|_| Err("timed out".to_string()))
}

/// Checks that the TLS service at `addr` serves a certificate that the CA in
/// `ca_pem` signed for `server_name`: the handshake succeeds.
///
/// # Panics
///
/// Panics when the handshake fails.
pub async fn assert_serves_certificate(addr: &str, server_name: &str, ca_pem: &str) {
    if let Err(e) = handshake(addr, server_name, ca_pem).await {
        panic!("security: TLS handshake with {addr} failed: {e}");
    }
}

/// Checks that the TLS service at `addr` refuses an expired certificate: the
/// handshake with it fails.
///
/// # Panics
///
/// Panics when the handshake succeeds.
pub async fn assert_rejects_expired_certificate(addr: &str, server_name: &str, ca_pem: &str) {
    if handshake(addr, server_name, ca_pem).await.is_ok() {
        panic!("security: {addr} accepted an expired certificate");
    }
}

/// Checks that the TLS service at `addr` changes its certificate on rotation.
/// The handshake must work, then `rotate` runs (the caller replaces the
/// certificate), then the handshake must work again with a certificate whose
/// DER bytes differ from the first one.
///
/// # Panics
///
/// Panics when a handshake fails, or when the certificate is the same after
/// rotation.
pub async fn assert_rotation(
    addr: &str,
    server_name: &str,
    ca_pem: &str,
    rotate: impl std::future::Future<Output = ()>,
) {
    let first = handshake(addr, server_name, ca_pem)
        .await
        .unwrap_or_else(|e| panic!("security: TLS handshake with {addr} failed: {e}"));
    rotate.await;
    let second = handshake(addr, server_name, ca_pem)
        .await
        .unwrap_or_else(|e| panic!("security: TLS handshake with {addr} failed: {e}"));
    if first == second {
        panic!("security: {addr} still serves the old certificate after rotation");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Serves forever on a local port. `answer` gets the `Authorization`
    /// header value (`None` when the request has none) and returns the status
    /// code of the response.
    fn serve(answer: fn(Option<&str>) -> u16) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&buf[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&request).into_owned();
                let authorization = head.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.trim()
                        .eq_ignore_ascii_case("authorization")
                        .then(|| value.trim())
                });
                let code = answer(authorization);
                let reason = match code {
                    200 => "OK",
                    401 => "Unauthorized",
                    403 => "Forbidden",
                    _ => "Status",
                };
                let response = format!(
                    "HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    /// Answers like a correct service: 401 with no credential, 403 for the
    /// `viewer` role, 200 for the `admin` role.
    fn correct(authorization: Option<&str>) -> u16 {
        match authorization {
            None => 401,
            Some("Bearer viewer") => 403,
            Some("Bearer admin") => 200,
            Some(_) => 401,
        }
    }

    fn accept_all(_: Option<&str>) -> u16 {
        200
    }

    fn reject_all(_: Option<&str>) -> u16 {
        401
    }

    #[tokio::test]
    async fn correct_server_passes_all_three_templates() {
        let base = serve(correct);
        assert_rejects_missing_credential(&base, "/x").await;
        assert_rejects_wrong_role(&base, "/x", "viewer").await;
        assert_accepts_credential(&base, "/x", "admin").await;
    }

    #[tokio::test]
    #[should_panic(expected = "without a credential returned 200, expected 401")]
    async fn accepts_a_missing_credential_fails_the_template() {
        let base = serve(accept_all);
        assert_rejects_missing_credential(&base, "/x").await;
    }

    #[tokio::test]
    #[should_panic(expected = "with a wrong role returned 200, expected 403")]
    async fn accepts_a_wrong_role_fails_the_template() {
        let base = serve(accept_all);
        assert_rejects_wrong_role(&base, "/x", "viewer").await;
    }

    #[tokio::test]
    #[should_panic(expected = "with a valid credential returned 401, expected 2xx")]
    async fn rejecting_a_valid_credential_fails_the_template() {
        let base = serve(reject_all);
        assert_accepts_credential(&base, "/x", "admin").await;
    }

    /// A CA that signs the leaves of the TLS tests: certificate and key.
    fn authority() -> (rcgen::Certificate, rcgen::KeyPair) {
        let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "e2e-ca");
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        (cert, key)
    }

    /// A leaf for `localhost` that the CA signs, valid from `not_before` to
    /// `not_after`. Returns the certificate DER and the PKCS8 key DER.
    fn leaf(
        ca: &(rcgen::Certificate, rcgen::KeyPair),
        not_before: time::OffsetDateTime,
        not_after: time::OffsetDateTime,
    ) -> (Vec<u8>, Vec<u8>) {
        let mut params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
        params.not_before = not_before;
        params.not_after = not_after;
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.signed_by(&key, &ca.0, &ca.1).unwrap();
        (cert.der().to_vec(), key.serialize_der())
    }

    fn at(instant: SystemTime) -> time::OffsetDateTime {
        let seconds = instant
            .duration_since(UNIX_EPOCH)
            .expect("after the epoch")
            .as_secs() as i64;
        time::OffsetDateTime::from_unix_timestamp(seconds).expect("representable instant")
    }

    /// From an hour ago to a day from now.
    fn valid_window() -> (time::OffsetDateTime, time::OffsetDateTime) {
        let now = SystemTime::now();
        (
            at(now - Duration::from_secs(3600)),
            at(now + Duration::from_secs(86_400)),
        )
    }

    /// From two days ago to one day ago.
    fn expired_window() -> (time::OffsetDateTime, time::OffsetDateTime) {
        let now = SystemTime::now();
        (
            at(now - Duration::from_secs(2 * 86_400)),
            at(now - Duration::from_secs(86_400)),
        )
    }

    /// Serves TLS on a local port. Each connection uses the certificate that
    /// `current` holds when the connection is accepted. Returns `host:port`.
    async fn tls_serve(current: Arc<Mutex<(Vec<u8>, Vec<u8>)>>) -> String {
        install_crypto_provider();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else {
                    continue;
                };
                let (cert, key) = current.lock().unwrap().clone();
                let config = rustls::ServerConfig::builder()
                    .with_no_client_auth()
                    .with_single_cert(
                        vec![rustls::pki_types::CertificateDer::from(cert)],
                        rustls::pki_types::PrivateKeyDer::try_from(key).unwrap(),
                    )
                    .unwrap();
                let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
                // A client that rejects the certificate fails this handshake;
                // that is the outcome under test, not a server error.
                let _ = acceptor.accept(tcp).await;
            }
        });
        format!("127.0.0.1:{port}")
    }

    #[tokio::test]
    async fn valid_certificate_passes_the_tls_templates() {
        let ca = authority();
        let (not_before, not_after) = valid_window();
        let current = Arc::new(Mutex::new(leaf(&ca, not_before, not_after)));
        let addr = tls_serve(current).await;
        assert_serves_certificate(&addr, "localhost", &ca.0.pem()).await;
    }

    #[tokio::test]
    #[should_panic(expected = "accepted an expired certificate")]
    async fn valid_certificate_fails_the_expired_template() {
        let ca = authority();
        let (not_before, not_after) = valid_window();
        let current = Arc::new(Mutex::new(leaf(&ca, not_before, not_after)));
        let addr = tls_serve(current).await;
        assert_rejects_expired_certificate(&addr, "localhost", &ca.0.pem()).await;
    }

    #[tokio::test]
    #[should_panic(expected = "TLS handshake with")]
    async fn expired_certificate_fails_the_handshake_template() {
        let ca = authority();
        let (not_before, not_after) = expired_window();
        let current = Arc::new(Mutex::new(leaf(&ca, not_before, not_after)));
        let addr = tls_serve(current).await;
        assert_serves_certificate(&addr, "localhost", &ca.0.pem()).await;
    }

    #[tokio::test]
    async fn expired_certificate_passes_the_rejects_template() {
        let ca = authority();
        let (not_before, not_after) = expired_window();
        let current = Arc::new(Mutex::new(leaf(&ca, not_before, not_after)));
        let addr = tls_serve(current).await;
        assert_rejects_expired_certificate(&addr, "localhost", &ca.0.pem()).await;
    }

    #[tokio::test]
    async fn rotation_passes_when_the_certificate_changes() {
        let ca = authority();
        let (not_before, not_after) = valid_window();
        let first = leaf(&ca, not_before, not_after);
        let second = leaf(&ca, not_before, not_after);
        let current = Arc::new(Mutex::new(first));
        let addr = tls_serve(current.clone()).await;
        let swap = current.clone();
        assert_rotation(&addr, "localhost", &ca.0.pem(), async move {
            *swap.lock().unwrap() = second;
        })
        .await;
    }

    #[tokio::test]
    #[should_panic(expected = "still serves the old certificate after rotation")]
    async fn rotation_fails_when_the_certificate_does_not_change() {
        let ca = authority();
        let (not_before, not_after) = valid_window();
        let current = Arc::new(Mutex::new(leaf(&ca, not_before, not_after)));
        let addr = tls_serve(current).await;
        assert_rotation(&addr, "localhost", &ca.0.pem(), async {}).await;
    }
}
