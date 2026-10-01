use crate::material::IdentityExpectation;

/// How a runtime's configs are built, and what its leaf must prove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsRuntimeProfile {
    pub identity: IdentityExpectation,
    pub alpn_protocols: Vec<Vec<u8>>,
    /// Require and verify a client certificate on the serving side.
    pub mutual: bool,
}

impl TlsRuntimeProfile {
    /// An ordinary public listener: ALPN offers `h2` and `http/1.1`, and clients
    /// are authenticated by their bearer token rather than by a certificate.
    ///
    /// Serving TLS proves *the server* and encrypts the token in flight; it is
    /// not an authorization input, and adding client-certificate requirements
    /// here would quietly turn it into one (#3112 R8).
    pub fn serving(dns_names: impl IntoIterator<Item = String>) -> Self {
        Self {
            identity: IdentityExpectation::serving(dns_names),
            alpn_protocols: vec![b"h2".to_vec(), b"http/1.1".to_vec()],
            mutual: false,
        }
    }

    /// A peer/replication port: mutual TLS, `h2` only.
    pub fn peer(
        dns_names: impl IntoIterator<Item = String>,
        spiffe_uris: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            identity: IdentityExpectation::peer(dns_names, spiffe_uris),
            alpn_protocols: vec![b"h2".to_vec()],
            mutual: true,
        }
    }
}
