//! The issuer that requests leaves from a CA Service pool.

use futures::future::BoxFuture;
use serde_json::{json, Value};

use super::{AccessTokenSource, CaPool};
use crate::domain::certificate::digest::hex_sha256;
use crate::domain::certificate::issuer::{
    IssuanceRequest, IssuedMaterial, Issuer, IssuerError, IssuerId,
};
use crate::infrastructure::certificate::secret_layout::parse_leaf;

/// Issues from a CA Service pool.
pub struct CasIssuer {
    id: IssuerId,
    pool: CaPool,
    tokens: Box<dyn AccessTokenSource>,
    client: reqwest::Client,
    endpoint: String,
}

impl CasIssuer {
    pub fn new(pool: CaPool, tokens: Box<dyn AccessTokenSource>) -> Self {
        Self {
            id: IssuerId::new(pool.resource()),
            pool,
            tokens,
            client: reqwest::Client::new(),
            endpoint: "https://privateca.googleapis.com/v1".to_string(),
        }
    }

    /// Point at another API endpoint. Tests only.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// `POST .../caPools/N/certificates?certificateId=…`
    pub fn certificates_url(&self, certificate_id: &str) -> String {
        format!(
            "{}/{}/certificates?certificateId={}",
            self.endpoint,
            self.pool.resource(),
            certificate_id
        )
    }

    /// The request body.
    ///
    /// Two fields, and the absences matter more than the presences. No
    /// `config` — that is the requester-supplied certificate description the
    /// pool refuses (`allow_config_based_issuance = false`, #3109). No
    /// `issuingCertificateAuthorityId` — the pool picks, so retiring a CA is a
    /// pool-level operation rather than a redeploy of everything that requests
    /// from it.
    pub fn request_body(request: &IssuanceRequest) -> Value {
        json!({
            "pemCsr": request.csr_pem,
            "lifetime": format!("{}s", request.lifetime.as_secs()),
        })
    }

    /// A certificate id derived from the CSR.
    ///
    /// Deterministic, so a retry after a timeout re-addresses the certificate
    /// the CA may already have issued instead of minting a sibling. Scoped by
    /// instance and purpose so two workloads cannot collide, and truncated to
    /// CA Service's 63-character limit.
    pub fn certificate_id(request: &IssuanceRequest) -> String {
        let fingerprint = hex_sha256(request.csr_pem.as_bytes());
        let id = format!(
            "{}-{}-{}",
            request.scope.instance,
            request.purpose.as_str(),
            &fingerprint[..16]
        );
        id.chars().take(63).collect()
    }

    async fn request(&self, request: IssuanceRequest) -> Result<IssuedMaterial, IssuerError> {
        let token = self.tokens.token().await?;
        let url = self.certificates_url(&Self::certificate_id(&request));
        let response = self
            .client
            .post(&url)
            .bearer_auth(token)
            .json(&Self::request_body(&request))
            .send()
            .await
            .map_err(|err| IssuerError::Upstream(format!("certificate request: {err}")))?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|err| IssuerError::Upstream(format!("certificate response: {err}")))?;
        if !status.is_success() {
            let reason = body["error"]["message"].as_str().unwrap_or("no detail");
            return Err(IssuerError::Upstream(format!(
                "certificate request rejected with {status}: {reason}"
            )));
        }
        Self::material(&self.id, &body)
    }

    /// Turn a CA Service response into material.
    ///
    /// Validity and fingerprint are parsed out of the returned certificate, not
    /// read from the response envelope. The certificate is the thing that will
    /// be presented; if the two ever disagreed, believing the envelope would
    /// mean scheduling renewal against a date nothing enforces.
    pub fn material(issuer: &IssuerId, body: &Value) -> Result<IssuedMaterial, IssuerError> {
        let certificate_pem = body["pemCertificate"]
            .as_str()
            .ok_or_else(|| IssuerError::Malformed("response carried no certificate".into()))?
            .to_string();
        let chain_pem = body["pemCertificateChain"]
            .as_array()
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|pem| pem.trim_end().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        let facts = parse_leaf(&certificate_pem).map_err(IssuerError::Malformed)?;
        Ok(IssuedMaterial {
            issuer: issuer.clone(),
            certificate_pem,
            chain_pem,
            not_before: facts.not_before,
            not_after: facts.not_after,
            fingerprint: facts.fingerprint,
        })
    }

    /// Fetch the pool's anchors.
    async fn anchors(&self) -> Result<String, IssuerError> {
        let token = self.tokens.token().await?;
        let url = format!("{}/{}:fetchCaCerts", self.endpoint, self.pool.resource());
        let response = self
            .client
            .post(&url)
            .bearer_auth(token)
            .json(&json!({}))
            .send()
            .await
            .map_err(|err| IssuerError::Upstream(format!("fetch CA certs: {err}")))?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|err| IssuerError::Upstream(format!("fetch CA certs response: {err}")))?;
        if !status.is_success() {
            return Err(IssuerError::Upstream(format!(
                "fetch CA certs rejected with {status}"
            )));
        }
        Ok(Self::anchors_from(&body))
    }

    /// Flatten a `fetchCaCerts` response into concatenated PEM.
    pub fn anchors_from(body: &Value) -> String {
        body["caCerts"]
            .as_array()
            .map(|chains| {
                chains
                    .iter()
                    .filter_map(|chain| chain["certificates"].as_array())
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|pem| pem.trim_end().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }
}

impl Issuer for CasIssuer {
    fn id(&self) -> IssuerId {
        self.id.clone()
    }

    fn issue<'a>(
        &'a self,
        request: IssuanceRequest,
    ) -> BoxFuture<'a, Result<IssuedMaterial, IssuerError>> {
        Box::pin(self.request(request))
    }

    fn trust_anchor_pem<'a>(&'a self) -> BoxFuture<'a, Result<String, IssuerError>> {
        Box::pin(self.anchors())
    }
}
