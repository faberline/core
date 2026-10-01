use async_trait::async_trait;
use jsonwebtoken::jwk::JwkSet;

use crate::application::google::JwksSource;
use crate::domain::google::{
    AccessTokenIntrospection, IntrospectedToken, GOOGLE_JWKS_URL, GOOGLE_TOKENINFO_URL,
};

// ---------------------------------------------------------------------------
// HTTP implementations
// ---------------------------------------------------------------------------

/// Fetches Google's published JWKS over HTTPS.
#[derive(Debug, Clone)]
pub struct HttpJwksSource {
    client: reqwest::Client,
    url: String,
}

impl HttpJwksSource {
    pub fn new(client: reqwest::Client, url: impl Into<String>) -> Self {
        Self {
            client,
            url: url.into(),
        }
    }

    pub fn google(client: reqwest::Client) -> Self {
        Self::new(client, GOOGLE_JWKS_URL)
    }
}

#[async_trait]
impl JwksSource for HttpJwksSource {
    async fn fetch(&self) -> Result<JwkSet, String> {
        let response = self
            .client
            .get(&self.url)
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?;
        if !response.status().is_success() {
            return Err(format!("JWKS endpoint returned {}", response.status()));
        }
        response
            .json::<JwkSet>()
            .await
            .map_err(|e| format!("JWKS response is not a key set: {}", e.without_url()))
    }
}

/// Calls Google's `tokeninfo` endpoint.
///
/// Every error is passed through [`reqwest::Error::without_url`] before it
/// becomes a string. The access token travels in the query string, so an error
/// that echoed its URL would put the caller's live credential into the
/// service's logs.
#[derive(Debug, Clone)]
pub struct HttpAccessTokenIntrospection {
    client: reqwest::Client,
    url: String,
}

impl HttpAccessTokenIntrospection {
    pub fn new(client: reqwest::Client, url: impl Into<String>) -> Self {
        Self {
            client,
            url: url.into(),
        }
    }

    pub fn google(client: reqwest::Client) -> Self {
        Self::new(client, GOOGLE_TOKENINFO_URL)
    }
}

#[async_trait]
impl AccessTokenIntrospection for HttpAccessTokenIntrospection {
    async fn introspect(&self, token: &str) -> Result<Option<IntrospectedToken>, String> {
        let response = self
            .client
            .get(&self.url)
            .query(&[("access_token", token)])
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?;

        // 400 is Google's "this token is not valid" answer — a verdict, not an
        // outage, so it must not become IntrospectionUnavailable.
        if response.status() == reqwest::StatusCode::BAD_REQUEST
            || response.status() == reqwest::StatusCode::UNAUTHORIZED
        {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!("tokeninfo returned {}", response.status()));
        }
        response
            .json::<IntrospectedToken>()
            .await
            .map(Some)
            .map_err(|e| format!("tokeninfo response was not understood: {}", e.without_url()))
    }
}
