//! Public CA bundle certificate validation.

/// Validation error for public trust bundle certificates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustBundleValidationError {
    /// Private key detected in the provided PEM string.
    PrivateKeyDetected,
    /// No valid PEM CERTIFICATE blocks were found.
    NoCertificatesFound,
    /// Invalid or unparseable PEM block encountered.
    InvalidBlock(String),
}

impl std::fmt::Display for TrustBundleValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrivateKeyDetected => {
                write!(f, "private key block detected; public trust bundle must contain only public certificates")
            }
            Self::NoCertificatesFound => {
                write!(f, "no PEM CERTIFICATE blocks found in trust bundle input")
            }
            Self::InvalidBlock(msg) => write!(f, "invalid PEM block: {msg}"),
        }
    }
}

impl std::error::Error for TrustBundleValidationError {}

const BEGIN_CERT: &str = "-----BEGIN CERTIFICATE-----";
const END_CERT: &str = "-----END CERTIFICATE-----";

/// Validates that `pem` contains only valid PEM `CERTIFICATE` blocks and no private keys.
///
/// Returns the list of parsed individual certificate PEM strings.
pub fn validate_public_certificates(pem: &str) -> Result<Vec<String>, TrustBundleValidationError> {
    let upper = pem.to_ascii_uppercase();
    if upper.contains("PRIVATE KEY") {
        return Err(TrustBundleValidationError::PrivateKeyDetected);
    }

    let mut certificates = Vec::new();
    let mut cursor = pem;

    while let Some(start_idx) = cursor.find(BEGIN_CERT) {
        let after_begin = &cursor[start_idx..];
        let end_idx = after_begin.find(END_CERT).ok_or_else(|| {
            TrustBundleValidationError::InvalidBlock("unterminated CERTIFICATE block".into())
        })?;

        let block_end = end_idx + END_CERT.len();
        let cert_block = after_begin[..block_end].trim();
        certificates.push(cert_block.to_string());

        cursor = &after_begin[block_end..];
    }

    if certificates.is_empty() {
        return Err(TrustBundleValidationError::NoCertificatesFound);
    }

    Ok(certificates)
}

/// Normalizes a slice of certificate PEM blocks into a canonical concatenated PEM string.
pub fn canonicalize_bundle_pem(certs: &[String]) -> String {
    let mut out = String::new();
    for cert in certs {
        let trimmed = cert.trim();
        if !trimmed.is_empty() {
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CERT_1: &str = "-----BEGIN CERTIFICATE-----\nMIIB...1\n-----END CERTIFICATE-----";
    const VALID_CERT_2: &str = "-----BEGIN CERTIFICATE-----\nMIIB...2\n-----END CERTIFICATE-----";
    const PRIV_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMIIE...key\n-----END PRIVATE KEY-----";

    #[test]
    fn validates_single_certificate() {
        let certs = validate_public_certificates(VALID_CERT_1).expect("valid");
        assert_eq!(certs.len(), 1);
        assert_eq!(certs[0], VALID_CERT_1);
    }

    #[test]
    fn validates_multiple_certificates() {
        let combined = format!("{VALID_CERT_1}\n{VALID_CERT_2}\n");
        let certs = validate_public_certificates(&combined).expect("valid");
        assert_eq!(certs.len(), 2);
    }

    #[test]
    fn rejects_private_key() {
        let err = validate_public_certificates(PRIV_KEY).unwrap_err();
        assert_eq!(err, TrustBundleValidationError::PrivateKeyDetected);

        let mixed = format!("{VALID_CERT_1}\n{PRIV_KEY}");
        let err = validate_public_certificates(&mixed).unwrap_err();
        assert_eq!(err, TrustBundleValidationError::PrivateKeyDetected);
    }

    #[test]
    fn rejects_empty_or_non_cert() {
        let err = validate_public_certificates("").unwrap_err();
        assert_eq!(err, TrustBundleValidationError::NoCertificatesFound);

        let err = validate_public_certificates("random junk with no cert").unwrap_err();
        assert_eq!(err, TrustBundleValidationError::NoCertificatesFound);
    }
}
