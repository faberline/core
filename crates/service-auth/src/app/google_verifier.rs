//! `GoogleVerifier::google`, wired to Google's HTTPS endpoints.

use std::sync::Arc;
use std::time::Duration;

use crate::application::google::GoogleVerifier;
use crate::application::role_map::ReloadableRoleMapVerifier;
use crate::domain::google::GoogleAuthConfig;
use crate::infrastructure::google::{HttpAccessTokenIntrospection, HttpJwksSource, SystemClock};

impl GoogleVerifier {
    /// Build a verifier against the real Google endpoints.
    pub fn google(
        required: bool,
        registry: Arc<ReloadableRoleMapVerifier>,
        config: GoogleAuthConfig,
    ) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        Self::with_sources(
            required,
            registry,
            config,
            Arc::new(HttpJwksSource::google(client.clone())),
            Some(Arc::new(HttpAccessTokenIntrospection::google(client))),
            Arc::new(SystemClock),
        )
    }
}
