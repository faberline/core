//! The X.509 adapter for the domain's [`LeafParser`] port.

use chrono::{TimeZone, Utc};

use crate::domain::certificate::digest::hex_sha256;
use crate::domain::certificate::projection::split_pem_blocks;
use crate::domain::certificate::secret_layout::{LeafFacts, LeafParser};

/// Reads a leaf with `x509_parser`.
#[derive(Clone, Copy, Debug, Default)]
pub struct X509LeafParser;

impl LeafParser for X509LeafParser {
    fn parse_leaf(&self, pem: &str) -> Result<LeafFacts, String> {
        parse_leaf(pem)
    }
}

/// Parse validity and fingerprint out of a PEM leaf.
pub fn parse_leaf(pem: &str) -> Result<LeafFacts, String> {
    let block = split_pem_blocks(pem)
        .into_iter()
        .next()
        .ok_or_else(|| "no PEM block".to_string())?;
    let der = pem_body_to_der(&block)?;
    let (_, cert) = x509_parser::parse_x509_certificate(&der)
        .map_err(|err| format!("parse certificate: {err}"))?;
    let not_before = Utc
        .timestamp_opt(cert.validity().not_before.timestamp(), 0)
        .single()
        .ok_or_else(|| "notBefore is not a representable instant".to_string())?;
    let not_after = Utc
        .timestamp_opt(cert.validity().not_after.timestamp(), 0)
        .single()
        .ok_or_else(|| "notAfter is not a representable instant".to_string())?;
    Ok(LeafFacts {
        not_before,
        not_after,
        fingerprint: hex_sha256(&der),
    })
}

fn pem_body_to_der(block: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    let body: String = block
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .map_err(|err| format!("decode PEM body: {err}"))
}
