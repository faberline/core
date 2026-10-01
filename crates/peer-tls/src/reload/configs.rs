use std::collections::HashSet;
use std::sync::Arc;

use rustls::pki_types::CertificateDer;
use rustls::{ClientConfig, RootCertStore, ServerConfig};

use super::TlsRuntimeProfile;
use crate::material::{Rejection, RejectionReason, ValidatedMaterial};

pub(super) fn build_configs(
    profile: &TlsRuntimeProfile,
    material: &ValidatedMaterial,
    carried: &[CertificateDer<'static>],
) -> Result<(Arc<ServerConfig>, Arc<ClientConfig>, usize), Rejection> {
    let anchors = dedupe(
        material
            .trust_anchors()
            .iter()
            .cloned()
            .chain(carried.iter().cloned()),
    );
    let mut roots = RootCertStore::empty();
    for anchor in &anchors {
        roots.add(anchor.clone()).map_err(|err| {
            Rejection::new(
                RejectionReason::MalformedPem,
                format!("trust anchor rejected: {err}"),
            )
        })?;
    }
    let roots = Arc::new(roots);

    let builder = ServerConfig::builder();
    let builder = if profile.mutual {
        let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::clone(&roots))
            .build()
            .map_err(|err| {
                Rejection::new(
                    RejectionReason::EmptyTrustBundle,
                    format!("client verifier: {err}"),
                )
            })?;
        builder.with_client_cert_verifier(verifier)
    } else {
        builder.with_no_client_auth()
    };
    let mut server = builder
        .with_single_cert(material.chain().to_vec(), material.key())
        .map_err(|err| Rejection::new(RejectionReason::KeyMismatch, err.to_string()))?;
    server.alpn_protocols = profile.alpn_protocols.clone();

    let mut client = ClientConfig::builder()
        .with_root_certificates((*roots).clone())
        .with_client_auth_cert(material.chain().to_vec(), material.key())
        .map_err(|err| Rejection::new(RejectionReason::KeyMismatch, err.to_string()))?;
    client.alpn_protocols = profile.alpn_protocols.clone();

    let count = anchors.len();
    Ok((Arc::new(server), Arc::new(client), count))
}

/// Preserve order, drop repeats. A bundle that already carries current+next and
/// a carried previous bundle overlap by construction; adding the same anchor
/// twice makes `RootCertStore::add` reject the second copy.
pub(super) fn dedupe(
    anchors: impl IntoIterator<Item = CertificateDer<'static>>,
) -> Vec<CertificateDer<'static>> {
    let mut seen: HashSet<Vec<u8>> = HashSet::new();
    let mut out = Vec::new();
    for anchor in anchors {
        if seen.insert(anchor.as_ref().to_vec()) {
            out.push(anchor);
        }
    }
    out
}
