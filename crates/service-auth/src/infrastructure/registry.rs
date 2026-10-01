use std::collections::HashMap;

use anyhow::{bail, Context as _, Result};

use crate::domain::authorization::{Registry, TokenClaims};

/// Load a credential registry from a registry-file path, for a service that
/// can resolve identity-keyed entries as well as bearer secrets.
///
/// Fails fast when `required` is true but the resolved registry ends up empty
/// in *both* namespaces (a server configured to mandate auth but unable to ever
/// authenticate anyone is a startup misconfiguration, not a per-request 401).
/// `registry_file_env` only words the error context; naming the actual env var
/// stays the caller's job, and a caller that knows a higher-level field
/// controls this should add that field to the error with `.context()`.
pub fn load_registry_file(
    required: bool,
    registry_file_env: &str,
    registry_file: Option<&str>,
) -> Result<Registry> {
    let registry = match registry_file {
        Some(path) if !path.trim().is_empty() => {
            let json = std::fs::read_to_string(path.trim())
                .with_context(|| format!("read {registry_file_env} `{}`", path.trim()))?;
            Registry::parse(&json)
                .with_context(|| format!("{registry_file_env} must contain JSON"))?
        }
        _ => Registry::default(),
    };
    if required && registry.is_empty() {
        bail!(
            "auth required but the credential registry is empty: point {registry_file_env} at a \
             file holding at least one `tokens` or `identities` entry"
        );
    }
    Ok(registry)
}

/// One place a registry may be projected from, and the env var that names it.
///
/// `env` only words the error messages; reading the variable stays the
/// caller's job so the message matches whatever the service calls it.
#[derive(Debug, Clone, Copy)]
pub struct RegistrySource<'a> {
    pub env: &'a str,
    pub path: Option<&'a str>,
}

/// Load a credential registry from several projected files at once, unioning
/// them.
///
/// The single-file [`load_registry_file`] assumes one Kubernetes object
/// carries the whole registry. That stops being true once the two namespaces
/// have different confidentiality classes: an `identities` map is ordinary
/// configuration (a ConfigMap), while a `tokens` map is a credential (a
/// Secret), and a deployment can reasonably have both (#2764). Each source is
/// parsed independently and [`Registry::try_merge`]d, so one malformed file
/// fails the load naming *that* file rather than silently serving a partial
/// registry.
///
/// Fails fast when `required` is true but every source is absent or empty: a
/// server told to mandate auth that can never authenticate anyone is a startup
/// misconfiguration, not a per-request 401.
pub fn load_registry_files(required: bool, sources: &[RegistrySource<'_>]) -> Result<Registry> {
    let mut registry = Registry::default();
    for source in sources {
        let Some(path) = source.path.map(str::trim).filter(|p| !p.is_empty()) else {
            continue;
        };
        let env = source.env;
        let json = std::fs::read_to_string(path).with_context(|| format!("read {env} `{path}`"))?;
        let parsed = Registry::parse(&json).with_context(|| format!("{env} must contain JSON"))?;
        registry
            .try_merge(parsed)
            .with_context(|| format!("merging {env} `{path}`"))?;
    }
    if required && registry.is_empty() {
        let names = sources
            .iter()
            .map(|source| source.env)
            .collect::<Vec<_>>()
            .join(" or ");
        bail!(
            "auth required but the credential registry is empty: point {names} at a file holding \
             at least one `tokens` or `identities` entry"
        );
    }
    Ok(registry)
}

/// Load a **bearer-only** token registry: a registry-file path (preferred,
/// production shape) or legacy inline JSON, in that priority order. Accepts
/// either document shape [`Registry::parse`] understands, but rejects one
/// carrying identity-keyed entries — a service wired to this loader has no
/// identity verifier and could never resolve them, so silently dropping them
/// would present as an unexplained 401. Fails fast when `required` is true but
/// the resolved registry ends up empty (a server configured to mandate auth but
/// unable to ever authenticate anyone is a startup misconfiguration, not a
/// per-request 401). `registry_file_env`/`legacy_tokens_env` only word the
/// error context; naming the actual env vars stays the caller's job so the
/// message matches whatever the service calls them.
pub fn load_registry(
    required: bool,
    registry_file_env: &str,
    registry_file: Option<&str>,
    legacy_tokens_env: &str,
    legacy_tokens_json: Option<&str>,
) -> Result<HashMap<String, TokenClaims>> {
    let (registry, source_env) = match registry_file {
        Some(path) if !path.trim().is_empty() => {
            let json = std::fs::read_to_string(path.trim())
                .with_context(|| format!("read {registry_file_env} `{}`", path.trim()))?;
            let registry = Registry::parse(&json)
                .with_context(|| format!("{registry_file_env} must contain JSON"))?;
            (registry, registry_file_env)
        }
        _ => match legacy_tokens_json {
            Some(json) if !json.trim().is_empty() => {
                let registry = Registry::parse(json)
                    .with_context(|| format!("{legacy_tokens_env} must be JSON"))?;
                (registry, legacy_tokens_env)
            }
            _ => (Registry::default(), registry_file_env),
        },
    };
    if !registry.identities.is_empty() {
        bail!(
            "{source_env} carries `identities` entries, which need an identity provider to \
             resolve; this service authenticates bearer secrets only"
        );
    }
    if required && registry.tokens.is_empty() {
        bail!(
            "auth required but no tokens: set a non-empty {registry_file_env} or {legacy_tokens_env}"
        );
    }
    Ok(registry.tokens)
}
