//! Static bearer-token auth config and per-resource authorize helper.

use std::collections::HashMap;
use std::result::Result as StdResult;
use std::sync::Arc;

use anyhow::{bail, Result};

use crate::application::http::AuthError;
use crate::application::role_map::ReloadableRoleMapVerifier;
use crate::domain::authorization::{AuditedRoleMapPrincipal, Role, TokenClaims};
use crate::infrastructure::{load_registry, TracingAuthEventSink};

/// The three environment-variable names a service uses, for messages and for `load_registry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthEnv {
    /// e.g. `RELAY_AUTH`
    pub mode: &'static str,
    /// e.g. `RELAY_TOKEN_REGISTRY_FILE`
    pub registry_file: &'static str,
    /// e.g. `RELAY_TOKENS`
    pub legacy_tokens: &'static str,
}

/// Resolved auth settings: the mode plus the token -> claims registry.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub required: bool,
    pub tokens: HashMap<String, TokenClaims>,
}

impl AuthConfig {
    /// Auth off, no tokens (tokenless dev).
    pub fn open() -> Self {
        Self {
            required: false,
            tokens: HashMap::new(),
        }
    }

    /// `mode` is `off`, `disabled` or `required` (trimmed, ASCII case-insensitive; empty = off).
    /// Any other mode fails naming `env.mode`. Then the registry is loaded through
    /// `load_registry`, which fails fast when auth is required and no tokens resolve.
    pub fn resolve(
        env: AuthEnv,
        mode: &str,
        registry_file: Option<&str>,
        legacy_tokens_json: Option<&str>,
    ) -> Result<Self> {
        let required = match mode.trim().to_ascii_lowercase().as_str() {
            "required" => true,
            "" | "off" | "disabled" => false,
            other => bail!(
                "{} must be `off`, `disabled`, or `required`; got `{other}`",
                env.mode
            ),
        };
        let tokens = load_registry(
            required,
            env.registry_file,
            registry_file,
            env.legacy_tokens,
            legacy_tokens_json,
        )?;
        Ok(Self { required, tokens })
    }

    /// The verifier the data-plane `auth_middleware` runs over this registry.
    pub fn verifier(&self) -> ReloadableRoleMapVerifier {
        ReloadableRoleMapVerifier::with_sink(
            self.required,
            self.tokens.clone(),
            Arc::new(TracingAuthEventSink),
        )
    }
}

/// Authorize `principal` for `needed` on `resource`. A denial becomes
/// `AuthError::Forbidden` with the shared 403 message.
pub fn authorize(
    principal: &AuditedRoleMapPrincipal,
    resource: &str,
    needed: Role,
) -> StdResult<(), AuthError> {
    principal.ensure(resource, needed).map_err(|denied| {
        AuthError::Forbidden(format!(
            "subject `{}` lacks {:?} on `{}`",
            denied.subject, denied.needed, denied.resource
        ))
    })
}
