use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, UnixTime};

use super::{Rejection, RejectionReason};

pub(super) fn parse_certificates(
    pem: &[u8],
    what: &str,
) -> Result<Vec<CertificateDer<'static>>, Rejection> {
    let certs = CertificateDer::pem_slice_iter(pem)
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
    PrivateKeyDer::from_pem_slice(pem).map_err(|err| match err {
        rustls::pki_types::pem::Error::NoItemsFound => Rejection::new(
            RejectionReason::MalformedPem,
            "no private key in the projected key material",
        ),
        other => Rejection::new(
            RejectionReason::MalformedPem,
            format!("private key: {other}"),
        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    const B64: &str = "AAAA";

    fn block(label: &str) -> Vec<u8> {
        format!("-----BEGIN {label}-----\n{B64}\n-----END {label}-----\n").into_bytes()
    }

    #[test]
    fn parse_private_key_reads_pkcs8() {
        let input = block("PRIVATE KEY");
        assert!(matches!(
            parse_private_key(&input).unwrap(),
            PrivateKeyDer::Pkcs8(_)
        ));
    }

    #[test]
    fn parse_private_key_reads_pkcs1() {
        let input = block("RSA PRIVATE KEY");
        assert!(matches!(
            parse_private_key(&input).unwrap(),
            PrivateKeyDer::Pkcs1(_)
        ));
    }

    #[test]
    fn parse_private_key_reads_sec1() {
        let input = block("EC PRIVATE KEY");
        assert!(matches!(
            parse_private_key(&input).unwrap(),
            PrivateKeyDer::Sec1(_)
        ));
    }

    #[test]
    fn parse_private_key_skips_a_certificate_block_before_the_key() {
        let input = [block("CERTIFICATE"), block("PRIVATE KEY")].concat();
        assert!(matches!(
            parse_private_key(&input).unwrap(),
            PrivateKeyDer::Pkcs8(_)
        ));
    }

    #[test]
    fn parse_private_key_rejects_input_with_no_key() {
        let input = block("CERTIFICATE");
        let err = parse_private_key(&input).unwrap_err();
        assert_eq!(err.reason, RejectionReason::MalformedPem);
        assert!(err.detail.contains("no private key"));
    }

    #[test]
    fn parse_certificates_reads_every_certificate_block() {
        let input = [block("CERTIFICATE"), block("CERTIFICATE")].concat();
        assert_eq!(
            parse_certificates(&input, "certificate chain")
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn parse_certificates_rejects_an_empty_chain() {
        let err = parse_certificates(b"", "certificate chain").unwrap_err();
        assert_eq!(err.reason, RejectionReason::MalformedPem);
        assert!(err.detail.contains("contains no certificate"));
    }

    #[test]
    fn parse_certificates_allows_an_empty_trust_bundle() {
        assert!(parse_certificates(b"", "trust bundle").unwrap().is_empty());
    }

    #[test]
    fn parse_certificates_rejects_bad_base64() {
        let input = b"-----BEGIN CERTIFICATE-----\n!!!!\n-----END CERTIFICATE-----\n";
        let err = parse_certificates(input, "certificate chain").unwrap_err();
        assert_eq!(err.reason, RejectionReason::MalformedPem);
    }
}
