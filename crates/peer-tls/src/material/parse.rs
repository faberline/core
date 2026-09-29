use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustls::pki_types::{CertificateDer, PrivateKeyDer, UnixTime};

use super::{Rejection, RejectionReason};

pub(super) fn parse_certificates(
    pem: &[u8],
    what: &str,
) -> Result<Vec<CertificateDer<'static>>, Rejection> {
    let mut reader = std::io::BufReader::new(pem);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| Rejection::new(RejectionReason::MalformedPem, format!("{what}: {err}")))?;
    if certs.is_empty() && what != "trust bundle" {
        return Err(Rejection::new(
            RejectionReason::MalformedPem,
            format!("{what} contains no certificate"),
        ));
    }
    Ok(certs)
}

pub(super) fn parse_private_key(pem: &[u8]) -> Result<PrivateKeyDer<'static>, Rejection> {
    let mut reader = std::io::BufReader::new(pem);
    rustls_pemfile::private_key(&mut reader)
        .map_err(|err| {
            Rejection::new(RejectionReason::MalformedPem, format!("private key: {err}"))
        })?
        .ok_or_else(|| {
            Rejection::new(
                RejectionReason::MalformedPem,
                "no private key in the projected key material",
            )
        })
}

pub(super) struct LeafFacts {
    pub(super) fingerprint: String,
    pub(super) not_before: SystemTime,
    pub(super) not_after: SystemTime,
    pub(super) uri_sans: Vec<String>,
}

pub(super) fn leaf_facts(leaf: &CertificateDer<'static>) -> Result<LeafFacts, Rejection> {
    let (_, parsed) = x509_parser::parse_x509_certificate(leaf.as_ref()).map_err(|err| {
        Rejection::new(
            RejectionReason::MalformedPem,
            format!("parse leaf certificate: {err}"),
        )
    })?;
    let not_before = from_unix_seconds(parsed.validity().not_before.timestamp())?;
    let not_after = from_unix_seconds(parsed.validity().not_after.timestamp())?;

    let mut uri_sans = Vec::new();
    if let Ok(Some(san)) = parsed.subject_alternative_name() {
        for name in &san.value.general_names {
            if let x509_parser::extensions::GeneralName::URI(uri) = name {
                uri_sans.push((*uri).to_string());
            }
        }
    }

    Ok(LeafFacts {
        fingerprint: hex_sha256(leaf.as_ref()),
        not_before,
        not_after,
        uri_sans,
    })
}

fn from_unix_seconds(seconds: i64) -> Result<SystemTime, Rejection> {
    if seconds < 0 {
        return Err(Rejection::new(
            RejectionReason::MalformedPem,
            "certificate validity predates the unix epoch",
        ));
    }
    Ok(UNIX_EPOCH + Duration::from_secs(seconds as u64))
}

pub(super) fn to_unix_time(now: SystemTime) -> UnixTime {
    UnixTime::since_unix_epoch(now.duration_since(UNIX_EPOCH).unwrap_or_default())
}

/// Lowercase hex sha256, no separators — the encoding
/// `service_k8s::certificate` writes into status.
fn hex_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
