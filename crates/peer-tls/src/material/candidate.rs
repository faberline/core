use std::fmt;

/// One projection's worth of PEM, as bytes.
///
/// Bytes rather than paths so that everything below this point is testable
/// without a filesystem, and so a caller that gets its material from a
/// Kubernetes watch instead of a mounted Secret needs no second code path.
#[derive(Clone, PartialEq, Eq)]
pub struct MaterialPem {
    pub cert_chain: Vec<u8>,
    pub key: Vec<u8>,
    pub trust_bundle: Vec<u8>,
}

impl MaterialPem {
    pub fn new(
        cert_chain: impl Into<Vec<u8>>,
        key: impl Into<Vec<u8>>,
        trust_bundle: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            cert_chain: cert_chain.into(),
            key: key.into(),
            trust_bundle: trust_bundle.into(),
        }
    }
}

/// Sizes only. A derived `Debug` here would print a private key into any log
/// line that formatted the enclosing struct (#3112 R6).
impl fmt::Debug for MaterialPem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MaterialPem")
            .field("cert_chain_bytes", &self.cert_chain.len())
            .field("key_bytes", &self.key.len())
            .field("trust_bundle_bytes", &self.trust_bundle.len())
            .finish()
    }
}

/// The identity a leaf must actually carry, and the roles it must be usable for.
///
/// Configured by the service, not read off the certificate: the point is to
/// catch the projection that swapped in a valid certificate for *something
/// else*. A leaf that chains correctly but names another workload is exactly
/// the case an "is it valid?" check waves through.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentityExpectation {
    /// Every name here must appear in the leaf's DNS SANs.
    pub dns_names: Vec<String>,
    /// Every URI here must appear in the leaf's URI SANs.
    pub spiffe_uris: Vec<String>,
    /// The leaf must be usable to authenticate a server.
    pub require_server_auth: bool,
    /// The leaf must be usable to authenticate a client.
    pub require_client_auth: bool,
}

impl IdentityExpectation {
    /// A serving leaf: presents these names to clients, never dials as a client.
    pub fn serving(dns_names: impl IntoIterator<Item = String>) -> Self {
        Self {
            dns_names: dns_names.into_iter().collect(),
            spiffe_uris: Vec::new(),
            require_server_auth: true,
            require_client_auth: false,
        }
    }

    /// A peer leaf: the same material both accepts and dials, so it must satisfy
    /// both roles or one direction of the mesh silently fails.
    pub fn peer(
        dns_names: impl IntoIterator<Item = String>,
        spiffe_uris: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            dns_names: dns_names.into_iter().collect(),
            spiffe_uris: spiffe_uris.into_iter().collect(),
            require_server_auth: true,
            require_client_auth: true,
        }
    }
}
