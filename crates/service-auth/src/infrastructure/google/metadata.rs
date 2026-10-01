use std::fmt;

use crate::domain::google::DEFAULT_METADATA_BASE_URL;

/// Typed error when fetching an ID token from the GCE/GKE metadata server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataTokenError {
    /// Non-2xx HTTP status from metadata server.
    HttpStatus {
        status: reqwest::StatusCode,
        base_url: String,
        message: String,
    },
    /// Network connection or transport failure contacting metadata server.
    ConnectionFailed { base_url: String, message: String },
}

impl fmt::Display for MetadataTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpStatus {
                status,
                base_url,
                message,
            } => {
                write!(
                    f,
                    "metadata server at {base_url} returned status {status}: {message}"
                )
            }
            Self::ConnectionFailed { base_url, message } => {
                write!(
                    f,
                    "could not connect to metadata server at {base_url}: {message}"
                )
            }
        }
    }
}

impl std::error::Error for MetadataTokenError {}

/// Source for ID tokens fetched from the GCE/GKE metadata server.
#[derive(Debug, Clone)]
pub struct MetadataTokenSource {
    client: reqwest::Client,
    base_url: String,
}

impl MetadataTokenSource {
    pub fn new(client: reqwest::Client, base_url: impl Into<String>) -> Self {
        Self {
            client,
            base_url: base_url.into(),
        }
    }

    pub fn default_gcp(client: reqwest::Client) -> Self {
        Self::new(client, DEFAULT_METADATA_BASE_URL)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn fetch_id_token(&self, audience: &str) -> Result<String, MetadataTokenError> {
        let url = format!(
            "{}/computeMetadata/v1/instance/service-accounts/default/identity",
            self.base_url.trim_end_matches('/')
        );
        let response = self
            .client
            .get(&url)
            .header("Metadata-Flavor", "Google")
            .query(&[("audience", audience), ("format", "full")])
            .send()
            .await
            .map_err(|e| MetadataTokenError::ConnectionFailed {
                base_url: self.base_url.clone(),
                message: e.without_url().to_string(),
            })?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| MetadataTokenError::ConnectionFailed {
                base_url: self.base_url.clone(),
                message: e.without_url().to_string(),
            })?;

        if !status.is_success() {
            return Err(MetadataTokenError::HttpStatus {
                status,
                base_url: self.base_url.clone(),
                message: text,
            });
        }

        Ok(text.trim().to_string())
    }
}

#[cfg(test)]
mod tests;
