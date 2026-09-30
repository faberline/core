//! The rcgen adapter for the domain's [`KeyAndCsrGenerator`] port.
//!
//! The key is generated here, in memory, and handed straight back to
//! [`crate::domain::certificate::issuer::IssuanceRequest::build`], which wraps
//! it in a `PrivateKey` before anything else can see it.

use crate::domain::certificate::issuer::{IssuerError, KeyAndCsrGenerator};
use crate::domain::certificate::profile::CertificateProfile;

/// Generates an in-memory P-256 keypair and a CSR carrying the profile's names.
///
/// P-256 rather than RSA: leaves here live for hours and are minted by a
/// controller that may be renewing several at once, so key generation cost is a
/// real operational property, not a benchmark curiosity.
#[derive(Clone, Copy, Debug, Default)]
pub struct RcgenCsrGenerator;

impl KeyAndCsrGenerator for RcgenCsrGenerator {
    fn generate(&self, profile: &CertificateProfile) -> Result<(String, String), IssuerError> {
        use rcgen::{CertificateParams, DnType, KeyPair, KeyUsagePurpose, SanType};

        let mut params = CertificateParams::default();
        params
            .distinguished_name
            .push(DnType::CommonName, profile.common_name());
        for name in &profile.identity().dns_names {
            let san = name
                .as_str()
                .try_into()
                .map(SanType::DnsName)
                .map_err(|err| IssuerError::KeyGeneration(format!("DNS SAN {name}: {err}")))?;
            params.subject_alt_names.push(san);
        }
        if let Some(uri) = &profile.identity().spiffe_uri {
            let san = uri
                .as_str()
                .try_into()
                .map(SanType::URI)
                .map_err(|err| IssuerError::KeyGeneration(format!("URI SAN {uri}: {err}")))?;
            params.subject_alt_names.push(san);
        }
        // The two basic usages a TLS leaf needs, and only those. `KeyCertSign`
        // and `CrlSign` are what would turn this into a CA in everything but
        // name, and the issuing pool refuses them anyway (#3109) -- stated on
        // both sides because a requester that asks for them should fail
        // locally, not at the CA, where the failure is a rate-limited API error
        // at 3am.
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];

        let key_pair = KeyPair::generate()
            .map_err(|err| IssuerError::KeyGeneration(format!("generate keypair: {err}")))?;
        let csr = params
            .serialize_request(&key_pair)
            .map_err(|err| IssuerError::KeyGeneration(format!("serialize CSR: {err}")))?;
        let csr_pem = csr
            .pem()
            .map_err(|err| IssuerError::KeyGeneration(format!("encode CSR: {err}")))?;
        Ok((key_pair.serialize_pem(), csr_pem))
    }
}

#[cfg(test)]
mod tests;
