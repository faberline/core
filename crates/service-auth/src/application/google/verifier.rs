use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use axum::http::HeaderMap;
use jsonwebtoken::{
    decode, decode_header, errors::ErrorKind as JwtErrorKind, Algorithm, Validation,
};
use serde::Deserialize;

use super::credential::{classify, Credential};
use super::introspection::{lenient_bool, AccessTokenIntrospection};
use super::jwks_cache::JwksCache;
use super::jwks_source::JwksSource;
use crate::application::http::{bearer_token, AsyncVerifier, AuthError};
use crate::application::role_map::ReloadableRoleMapVerifier;
use crate::domain::authorization::{AuditedRoleMapPrincipal, RoleMapPrincipal};
use crate::domain::google::{
    Clock, GoogleAuthConfig, GoogleAuthError, InvalidReason, GOOGLE_ISSUERS,
};

// ---------------------------------------------------------------------------
// Introspection cache
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CachedIdentity {
    email: String,
    expires_at_unix: u64,
}

// ---------------------------------------------------------------------------
// The verifier
// ---------------------------------------------------------------------------

/// A [`AsyncVerifier`] that accepts Google identities and pre-shared bearer
/// secrets, and authorizes both through one unmodified role map.
pub struct GoogleVerifier {
    required: bool,
    registry: Arc<ReloadableRoleMapVerifier>,
    validation: Validation,
    jwks: JwksCache,
    introspection: Option<Arc<dyn AccessTokenIntrospection>>,
    introspection_ttl_ceiling: Duration,
    introspection_cache: RwLock<HashMap<String, CachedIdentity>>,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for GoogleVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No cache contents: its keys are live credentials.
        f.debug_struct("GoogleVerifier")
            .field("required", &self.required)
            .field("introspection", &self.introspection.is_some())
            .field("registry_revision", &self.registry.revision())
            .finish_non_exhaustive()
    }
}

impl GoogleVerifier {
    /// Build a verifier over injected sources — the constructor tests use, and
    /// the one a service uses to disable the introspection path by passing
    /// `None`.
    ///
    /// Fails on an empty audience list. `jsonwebtoken` treats an empty
    /// `set_audience` as "do not check the audience", so the one mistake this
    /// rejects — omitting the audience — would otherwise turn every ID token
    /// minted for any other Google-fronted service into a valid credential
    /// here, and it would do so silently.
    pub fn with_sources(
        required: bool,
        registry: Arc<ReloadableRoleMapVerifier>,
        config: GoogleAuthConfig,
        jwks: Arc<dyn JwksSource>,
        introspection: Option<Arc<dyn AccessTokenIntrospection>>,
        clock: Arc<dyn Clock>,
    ) -> anyhow::Result<Self> {
        if config.audiences.iter().all(|a| a.trim().is_empty()) {
            anyhow::bail!(
                "Google identity verification needs at least one audience: an ID-token \
                 verifier with no audience accepts tokens minted for someone else's service"
            );
        }
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&config.audiences);
        validation.set_issuer(&GOOGLE_ISSUERS);
        // Validated by default; stated so the intent survives a refactor.
        validation.validate_exp = true;

        Ok(Self {
            required,
            registry,
            validation,
            jwks: JwksCache::new(jwks, Arc::clone(&clock), config.jwks_refetch_min_interval),
            introspection,
            introspection_ttl_ceiling: config.introspection_ttl_ceiling,
            introspection_cache: RwLock::new(HashMap::new()),
            clock,
        })
    }

    /// Verify a Google ID token offline and return its verified email.
    pub async fn verify_id_token(&self, token: &str) -> Result<String, GoogleAuthError> {
        let header = decode_header(token)
            .map_err(|e| GoogleAuthError::MalformedToken(format!("{:?}", e.kind())))?;
        let kid = header.kid.ok_or_else(|| {
            GoogleAuthError::MalformedToken(
                "no kid; key rotation would be unresolvable without one".to_string(),
            )
        })?;
        let key = self.jwks.decoding_key(&kid).await?;
        let data = decode::<GoogleIdClaims>(token, &key, &self.validation)
            .map_err(|e| classify_jwt_error(&e))?;
        verified_email(data.claims.email, data.claims.email_verified)
    }

    /// Resolve an opaque access token to a verified email by asking Google,
    /// serving a cached answer when one is still live.
    pub async fn introspect_access_token(&self, token: &str) -> Result<String, GoogleAuthError> {
        if let Some(email) = self.cached_identity(token) {
            return Ok(email);
        }
        let introspection = self
            .introspection
            .as_ref()
            .ok_or(GoogleAuthError::IntrospectionNotConfigured)?;

        let introspected = introspection
            .introspect(token)
            .await
            .map_err(GoogleAuthError::IntrospectionUnavailable)?
            .ok_or(GoogleAuthError::Rejected)?;

        let email = verified_email(introspected.email, introspected.email_verified)?;
        self.cache_identity(token, &email, introspected.expires_in);
        Ok(email)
    }

    fn cached_identity(&self, token: &str) -> Option<String> {
        let now = self.clock.now_unix();
        let cache = self
            .introspection_cache
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache
            .get(token)
            .filter(|entry| entry.expires_at_unix > now)
            .map(|entry| entry.email.clone())
    }

    fn cache_identity(&self, token: &str, email: &str, expires_in: u64) {
        let ttl = expires_in.min(self.introspection_ttl_ceiling.as_secs());
        if ttl == 0 {
            return;
        }
        let expires_at_unix = self.clock.now_unix().saturating_add(ttl);
        let mut cache = self
            .introspection_cache
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Bound the map by dropping whatever has already lapsed; the cache is
        // keyed by credential, so it must not accumulate one entry per token
        // ever presented.
        let now = self.clock.now_unix();
        cache.retain(|_, entry| entry.expires_at_unix > now);
        cache.insert(
            token.to_string(),
            CachedIdentity {
                email: email.to_string(),
                expires_at_unix,
            },
        );
    }

    /// The shared tail both Google paths converge on: a verified identity is
    /// looked up in the registry, and authentication succeeding is not
    /// authorization succeeding.
    fn resolve(&self, identity: &str) -> Result<RoleMapPrincipal, GoogleAuthError> {
        self.registry
            .lookup_identity(identity)
            .map(RoleMapPrincipal::Token)
            .ok_or(GoogleAuthError::NotInRegistry)
    }

    /// Authenticate a presented credential, returning the typed reason on
    /// failure. [`AsyncVerifier::authenticate_async`] is this, with the reason
    /// narrowed to an [`AuthError`].
    pub async fn authenticate_credential(
        &self,
        token: &str,
    ) -> Result<RoleMapPrincipal, GoogleAuthError> {
        match classify(token) {
            Credential::GoogleIdToken => {
                let email = self.verify_id_token(token).await?;
                self.resolve(&email)
            }
            Credential::Opaque => {
                // Registry first: a pre-shared secret is a local map hit and
                // must not acquire a network round trip. `lookup_secret`, not
                // `lookup_identity` — the bearer namespace is the only one a
                // presented string may reach (#2678, R1).
                if let Some(claims) = self.registry.lookup_secret(token) {
                    return Ok(RoleMapPrincipal::Token(claims));
                }
                let email = self.introspect_access_token(token).await?;
                self.resolve(&email)
            }
        }
    }
}

/// Only the claims this design reads. Google sends more; ignoring them is
/// deliberate — every claim consumed becomes contract.
#[derive(Debug, Deserialize)]
struct GoogleIdClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(default, deserialize_with = "lenient_bool")]
    email_verified: bool,
}

fn verified_email(email: Option<String>, verified: bool) -> Result<String, GoogleAuthError> {
    let email = email
        .filter(|e| !e.trim().is_empty())
        .ok_or(GoogleAuthError::EmailMissing)?;
    if !verified {
        return Err(GoogleAuthError::EmailUnverified);
    }
    Ok(email)
}

#[async_trait]
impl AsyncVerifier for GoogleVerifier {
    /// The same audited principal the bearer-only path produces.
    ///
    /// The wrapper is not decoration: `ensure` is what emits the
    /// allow/deny audit event, so a Google-authenticated caller whose
    /// principal skipped it would authorize invisibly while an identical
    /// bearer-authenticated caller stayed on the record.
    type Principal = AuditedRoleMapPrincipal;

    async fn authenticate_async(
        &self,
        headers: &HeaderMap,
    ) -> Result<AuditedRoleMapPrincipal, AuthError> {
        let principal = match (self.required, bearer_token(headers)) {
            (false, None) => RoleMapPrincipal::Open,
            (_, Some(token)) => self.authenticate_credential(token).await.map_err(|error| {
                if error.is_upstream_failure() {
                    tracing::warn!(
                        target: "service_auth.audit",
                        event = "identity_provider_unavailable",
                        reason = %error,
                    );
                }
                AuthError::from(error)
            })?,
            (true, None) => return Err(AuthError::Unauthenticated),
        };
        Ok(self.registry.audited(principal))
    }

    fn required(&self) -> bool {
        self.required
    }
}

impl From<GoogleAuthError> for AuthError {
    fn from(error: GoogleAuthError) -> Self {
        if error.is_upstream_failure() {
            // The message is built from upstream error text with the URL
            // stripped, so it cannot carry the presented credential.
            AuthError::Unavailable(error.to_string())
        } else {
            AuthError::Unauthenticated
        }
    }
}

fn classify_jwt_error(error: &jsonwebtoken::errors::Error) -> GoogleAuthError {
    match error.kind() {
        JwtErrorKind::InvalidAudience => GoogleAuthError::Invalid(InvalidReason::Audience),
        JwtErrorKind::InvalidIssuer => GoogleAuthError::Invalid(InvalidReason::Issuer),
        JwtErrorKind::ExpiredSignature => GoogleAuthError::Invalid(InvalidReason::Expired),
        JwtErrorKind::InvalidSignature => GoogleAuthError::Invalid(InvalidReason::Signature),
        other => GoogleAuthError::MalformedToken(format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests;
