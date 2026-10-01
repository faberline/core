use std::sync::Arc;
use std::time::SystemTime;

use rustls::client::danger::ServerCertVerifier;
use rustls::client::WebPkiServerVerifier;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::{ParsedCertificate, WebPkiClientVerifier};
use rustls::{CertificateError, RootCertStore};

use super::parse::{leaf_facts, parse_certificates, parse_private_key, to_unix_time};
use super::{IdentityExpectation, MaterialPem, Rejection, RejectionReason, ValidatedMaterial};
use crate::install_default_crypto_provider;

/// Parse and check a candidate against `expect` as of `now`.
///
/// The order is deliberate: cheap structural failures first, so an operator who
/// mounted the wrong file gets "malformed_pem" rather than a chain error that
/// reads like a CA problem.
pub fn validate(
    pem: &MaterialPem,
    expect: &IdentityExpectation,
    now: SystemTime,
) -> Result<ValidatedMaterial, Rejection> {
    install_default_crypto_provider();

    let chain = parse_certificates(&pem.cert_chain, "certificate chain")?;
    let trust = parse_certificates(&pem.trust_bundle, "trust bundle")?;
    if trust.is_empty() {
        return Err(Rejection::new(
            RejectionReason::EmptyTrustBundle,
            "trust bundle contains no anchors",
        ));
    }
    let key = parse_private_key(&pem.key)?;

    let leaf = chain
        .first()
        .cloned()
        .ok_or_else(|| Rejection::new(RejectionReason::MalformedPem, "empty certificate chain"))?;
    let intermediates = &chain[1..];

    // The key must belong to the leaf. rustls answers "unknown" for key types it
    // cannot introspect; that is not evidence of a mismatch, so it is not
    // treated as one — `from_der` already encodes exactly that distinction.
    let provider = rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .ok_or_else(|| {
            Rejection::new(
                RejectionReason::MalformedPem,
                "no process-default crypto provider is installed",
            )
        })?;
    rustls::sign::CertifiedKey::from_der(chain.clone(), key.clone_key(), &provider)
        .map_err(|err| Rejection::new(RejectionReason::KeyMismatch, err.to_string()))?;

    let facts = leaf_facts(&leaf)?;
    // Checked before the verifiers so the reported reason is the operator's
    // actual problem: webpki collapses both edges of the window into one error
    // family, and "expired" versus "not yet valid" is the difference between a
    // stalled controller and a clock skew.
    if now < facts.not_before {
        return Err(Rejection::new(
            RejectionReason::NotYetValid,
            "leaf notBefore is in the future",
        ));
    }
    if now >= facts.not_after {
        return Err(Rejection::new(
            RejectionReason::Expired,
            "leaf notAfter is in the past",
        ));
    }

    let mut roots = RootCertStore::empty();
    for anchor in &trust {
        roots.add(anchor.clone()).map_err(|err| {
            Rejection::new(
                RejectionReason::MalformedPem,
                format!("trust anchor rejected: {err}"),
            )
        })?;
    }
    let roots = Arc::new(roots);
    let unix_now = to_unix_time(now);

    if expect.require_client_auth {
        let verifier = WebPkiClientVerifier::builder(Arc::clone(&roots))
            .build()
            .map_err(|err| {
                Rejection::new(
                    RejectionReason::EmptyTrustBundle,
                    format!("client verifier: {err}"),
                )
            })?;
        verifier
            .verify_client_cert(&leaf, intermediates, unix_now)
            .map_err(classify)?;
    }

    if expect.require_server_auth {
        let verifier = WebPkiServerVerifier::builder(Arc::clone(&roots))
            .build()
            .map_err(|err| {
                Rejection::new(
                    RejectionReason::EmptyTrustBundle,
                    format!("server verifier: {err}"),
                )
            })?;
        // Every configured name, not just the first: a leaf reissued for a
        // shrunken topology still verifies against one member's name, and
        // stopping at the first success is how the fleet finds out at dial time.
        let names = if expect.dns_names.is_empty() {
            Vec::new()
        } else {
            expect.dns_names.clone()
        };
        if names.is_empty() {
            verify_chain_only(&verifier, &leaf, intermediates, unix_now)?;
        }
        for name in names {
            let server_name = ServerName::try_from(name.clone()).map_err(|err| {
                Rejection::new(
                    RejectionReason::WrongIdentity,
                    format!("expected name `{name}` is not a valid server name: {err}"),
                )
            })?;
            verifier
                .verify_server_cert(&leaf, intermediates, &server_name, &[], unix_now)
                .map_err(|err| {
                    let mut rejection = classify(err);
                    if rejection.reason == RejectionReason::WrongIdentity {
                        rejection.detail = format!("leaf does not carry the name `{name}`");
                    }
                    rejection
                })?;
        }
    } else {
        // No serverAuth requirement, but the names still have to be on the leaf.
        let parsed = ParsedCertificate::try_from(&leaf)
            .map_err(|err| Rejection::new(RejectionReason::MalformedPem, err.to_string()))?;
        for name in &expect.dns_names {
            let server_name = ServerName::try_from(name.clone()).map_err(|err| {
                Rejection::new(
                    RejectionReason::WrongIdentity,
                    format!("expected name `{name}` is not a valid server name: {err}"),
                )
            })?;
            rustls::client::verify_server_name(&parsed, &server_name).map_err(|_| {
                Rejection::new(
                    RejectionReason::WrongIdentity,
                    format!("leaf does not carry the name `{name}`"),
                )
            })?;
        }
    }

    for uri in &expect.spiffe_uris {
        if !facts.uri_sans.iter().any(|san| san == uri) {
            return Err(Rejection::new(
                RejectionReason::WrongIdentity,
                format!("leaf does not carry the SPIFFE identity `{uri}`"),
            ));
        }
    }

    Ok(ValidatedMaterial {
        chain,
        key,
        trust,
        fingerprint: facts.fingerprint,
        not_before: facts.not_before,
        not_after: facts.not_after,
    })
}

/// Chain-only verification when no name was configured. `localhost` is a name
/// webpki will always parse; the verifier's name check is the only part of the
/// result deliberately ignored here, and every other failure still propagates.
fn verify_chain_only(
    verifier: &Arc<WebPkiServerVerifier>,
    leaf: &CertificateDer<'static>,
    intermediates: &[CertificateDer<'static>],
    now: UnixTime,
) -> Result<(), Rejection> {
    let placeholder = ServerName::try_from("localhost").expect("`localhost` is a valid DNS name");
    match verifier.verify_server_cert(leaf, intermediates, &placeholder, &[], now) {
        Ok(_) => Ok(()),
        Err(rustls::Error::InvalidCertificate(
            CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
        )) => Ok(()),
        Err(err) => Err(classify(err)),
    }
}

/// Map a rustls verification failure onto the reason an operator can act on.
fn classify(err: rustls::Error) -> Rejection {
    let detail = err.to_string();
    let reason =
        match &err {
            rustls::Error::InvalidCertificate(certificate) => match certificate {
                CertificateError::Expired | CertificateError::ExpiredContext { .. } => {
                    RejectionReason::Expired
                }
                CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => {
                    RejectionReason::NotYetValid
                }
                CertificateError::NotValidForName
                | CertificateError::NotValidForNameContext { .. } => RejectionReason::WrongIdentity,
                CertificateError::InvalidPurpose
                | CertificateError::InvalidPurposeContext { .. } => RejectionReason::MissingUsage,
                CertificateError::BadEncoding => RejectionReason::MalformedPem,
                _ => RejectionReason::Untrusted,
            },
            _ => RejectionReason::Untrusted,
        };
    Rejection::new(reason, detail)
}
