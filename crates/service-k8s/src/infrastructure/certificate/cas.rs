//! Asking GCP CA Service for a certificate.
//!
//! Optional, and that is structural rather than tidiness: everything that makes
//! this lifecycle correct — when to renew, what order to publish things in, what
//! to refuse — lives in modules that have never heard of GCP. This one turns a
//! CSR into an HTTP request and a response into a leaf. Under
//! `--no-default-features` it is not compiled, and the lifecycle still builds,
//! still tests, and still issues certificates through
//! [`super::ephemeral::EphemeralIssuer`] (R8).
//!
//! ### Credentials
//!
//! There are no long-lived keys or ADC files to hold. Access tokens are obtained
//! dynamically using two shared-library adapters:
//!
//! - [`GkeMetadataTokenSource`]: Lumen's Standard-GKE production path. It asks the
//!   fixed GKE metadata server endpoint for a short-lived OAuth access token using
//!   the Pod's KSA identity;
//! - [`WorkloadIdentityTokenSource`]: A compatibility adapter for other consumers
//!   that directly exchange a projected KSA token at GCP STS.
//!
//! Neither adapter uses a service-account key, credential Secret, or logs token
//! or response bodies. The pool's IAM binding names the workload's `principal://`
//! directly (#3109), so the KSA *is* the identity.
//!
//! ### Retries do not mint duplicates
//!
//! The certificate id is derived from the CSR. A request that timed out after
//! the CA had already signed comes back with the same id, and CA Service
//! returns the existing certificate rather than issuing a second one. Without
//! that, every network hiccup during renewal would leave a stray valid leaf
//! behind — valid, unreferenced, and counted against nothing.

use futures::future::BoxFuture;

use crate::domain::certificate::issuer::IssuerError;

mod cas_issuer;
mod token_source;

pub use cas_issuer::CasIssuer;
pub use token_source::{GkeMetadataTokenSource, WorkloadIdentityTokenSource};

const GKE_METADATA_TOKEN_ENDPOINT: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

/// The pool a certificate is requested from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaPool {
    pub project: String,
    pub location: String,
    pub pool: String,
}

impl CaPool {
    /// Parse `projects/P/locations/L/caPools/N` — the exact string #3109's
    /// Terraform emits as an output, so the operator's configuration is a copy
    /// rather than four fields someone reassembles by hand.
    pub fn parse(resource: &str) -> Result<Self, IssuerError> {
        let parts: Vec<&str> = resource.split('/').collect();
        match parts.as_slice() {
            ["projects", project, "locations", location, "caPools", pool]
                if !project.is_empty() && !location.is_empty() && !pool.is_empty() =>
            {
                for part in [*project, *location, *pool] {
                    if part.chars().any(|c| {
                        c.is_control() || matches!(c, ' ' | '"' | '\'' | '\\' | '\n' | '\r')
                    }) {
                        return Err(IssuerError::Upstream(format!(
                            "unsafe character in CA pool resource name: {resource}"
                        )));
                    }
                    if !part
                        .chars()
                        .all(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_'))
                    {
                        return Err(IssuerError::Upstream(format!(
                            "invalid segment in CA pool resource name: {resource}"
                        )));
                    }
                }
                Ok(Self {
                    project: (*project).to_string(),
                    location: (*location).to_string(),
                    pool: (*pool).to_string(),
                })
            }
            _ => Err(IssuerError::Upstream(format!(
                "not a CA pool resource name: {resource}"
            ))),
        }
    }

    pub fn resource(&self) -> String {
        format!(
            "projects/{}/locations/{}/caPools/{}",
            self.project, self.location, self.pool
        )
    }
}

/// Anything that can produce a bearer token for `privateca.googleapis.com`.
pub trait AccessTokenSource: Send + Sync {
    fn token<'a>(&'a self) -> BoxFuture<'a, Result<String, IssuerError>>;
}

#[cfg(test)]
mod tests;
