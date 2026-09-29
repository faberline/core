use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::{bail, Context, Result};
use axum::http::HeaderMap;

use crate::application::http::{bearer_token, AuthError, Verifier};
use crate::domain::authorization::{
    AuditedRoleMapPrincipal, AuthEvent, AuthEventSink, AuthorizationDecision, AuthorizationReason,
    NoopAuthEventSink, Registry, ReloadFailure, RoleMapPrincipal, TokenClaims,
};

#[derive(Clone)]
struct RegistrySnapshot {
    revision: u64,
    registry: Registry,
}

/// Role-map verifier whose registry can be replaced atomically after startup.
#[derive(Clone)]
pub struct ReloadableRoleMapVerifier {
    required: bool,
    snapshot: Arc<RwLock<RegistrySnapshot>>,
    sink: Arc<dyn AuthEventSink>,
    /// Subjects this service presents on its own behalf, which a tenant
    /// registry may therefore not claim (#2679, R4).
    reserved_subjects: Arc<Vec<String>>,
}

impl fmt::Debug for ReloadableRoleMapVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReloadableRoleMapVerifier")
            .field("required", &self.required)
            .field("revision", &self.revision())
            .field("entries", &self.entry_count())
            .finish_non_exhaustive()
    }
}

impl ReloadableRoleMapVerifier {
    pub fn new(required: bool, tokens: HashMap<String, TokenClaims>) -> Self {
        Self::with_sink(required, tokens, Arc::new(NoopAuthEventSink))
    }

    pub fn with_sink(
        required: bool,
        tokens: HashMap<String, TokenClaims>,
        sink: Arc<dyn AuthEventSink>,
    ) -> Self {
        Self::with_registry_and_sink(required, Registry::from_tokens(tokens), sink)
    }

    /// Seed from a two-namespace [`Registry`] (#2678), for a service that also
    /// resolves provider-verified identities.
    pub fn with_registry(required: bool, registry: Registry) -> Self {
        Self::with_registry_and_sink(required, registry, Arc::new(NoopAuthEventSink))
    }

    pub fn with_registry_and_sink(
        required: bool,
        registry: Registry,
        sink: Arc<dyn AuthEventSink>,
    ) -> Self {
        Self {
            required,
            snapshot: Arc::new(RwLock::new(RegistrySnapshot {
                revision: 0,
                registry,
            })),
            sink,
            reserved_subjects: Arc::new(Vec::new()),
        }
    }

    /// Reserve subjects the service presents on its own behalf, so no adopted
    /// registry may claim them (#2679, R4).
    ///
    /// lumen's control plane names itself in every admin call it makes. If a
    /// tenant registry could grant that same subject, the operator's calls and
    /// a tenant's calls would be indistinguishable in audit output — the one
    /// thing an attributable control-plane identity exists to prevent. This is
    /// a builder rather than a constructor argument so the reservation is
    /// visible at the call site that makes it.
    #[must_use]
    pub fn reserving_subjects(mut self, subjects: impl IntoIterator<Item = String>) -> Self {
        self.reserved_subjects = Arc::new(subjects.into_iter().collect());
        self
    }

    /// The subjects reserved by [`Self::reserving_subjects`].
    pub fn reserved_subjects(&self) -> &[String] {
        &self.reserved_subjects
    }

    pub fn open() -> Self {
        Self::new(false, HashMap::new())
    }

    /// Wrap an already-resolved principal in this verifier's audit sink.
    ///
    /// [`Verifier::authenticate`] does this internally, but a verifier that
    /// resolves credentials some other way — [`crate::gcp::GoogleVerifier`]
    /// asks an identity provider — needs the same wrapper so its principals
    /// emit the same authorization events. Without it the Google paths would
    /// authorize silently while the bearer path stayed audited.
    pub fn audited(&self, principal: RoleMapPrincipal) -> AuditedRoleMapPrincipal {
        AuditedRoleMapPrincipal {
            inner: principal,
            sink: Arc::clone(&self.sink),
        }
    }

    pub fn revision(&self) -> u64 {
        self.read_snapshot().revision
    }

    /// Entries across both namespaces in the currently adopted snapshot.
    pub fn entry_count(&self) -> usize {
        self.read_snapshot().registry.len()
    }

    /// Look a **bearer secret** up in the currently adopted snapshot.
    ///
    /// For verifiers that reach the registry outside [`Verifier::authenticate`]
    /// — [`crate::gcp::GoogleVerifier`] checks a pre-shared secret here before
    /// spending a network round trip on introspection. Routing it through the
    /// snapshot, instead of holding a separate map, is what keeps such a
    /// verifier subject to the same atomic rotation and last-known-good
    /// guarantees as `authenticate`.
    ///
    /// The returned value is cloned so the caller cannot hold the read lock
    /// across its own work.
    pub fn lookup_secret(&self, token: &str) -> Option<TokenClaims> {
        self.read_snapshot().registry.tokens.get(token).cloned()
    }

    /// Look a **provider-verified identity** up in the currently adopted
    /// snapshot: [`crate::gcp::GoogleVerifier`] turns a Google credential into
    /// a verified email and lands here.
    ///
    /// Deliberately a different map from [`Self::lookup_secret`] (#2678, R1):
    /// sharing one would let a bearer secret whose text happens to be a valid
    /// email match an identity entry and silently inherit its grants.
    pub fn lookup_identity(&self, identity: &str) -> Option<TokenClaims> {
        self.read_snapshot()
            .registry
            .identities
            .get(identity)
            .cloned()
    }

    /// Parse, validate, and atomically adopt an inline registry document.
    pub fn reload_json(&self, json: &str) -> Result<u64> {
        let registry = match Registry::parse(json) {
            Ok(registry) => registry,
            Err(error) => {
                self.record_reload_failure(ReloadFailure::Parse);
                return Err(error).context("replacement credential registry rejected");
            }
        };
        self.reload_registry(registry)
    }

    /// Read, parse, validate, and atomically adopt a registry file.
    pub fn reload_file(&self, path: impl AsRef<Path>) -> Result<u64> {
        self.reload_files(std::slice::from_ref(&path.as_ref().to_owned()))
    }

    /// Re-read every file the registry is projected from, union them, and
    /// adopt the result as one snapshot.
    ///
    /// All-or-nothing on purpose: when `identities` and `tokens` arrive from
    /// different Kubernetes objects (#2764), adopting only the file that
    /// changed would drop the other namespace entirely. A read or parse
    /// failure on any single file leaves the previous snapshot serving.
    pub fn reload_files(&self, paths: &[PathBuf]) -> Result<u64> {
        let mut registry = Registry::default();
        for path in paths {
            let json = match std::fs::read_to_string(path) {
                Ok(json) => json,
                Err(error) => {
                    self.record_reload_failure(ReloadFailure::Read);
                    return Err(error).with_context(|| format!("read registry {}", path.display()));
                }
            };
            let parsed = match Registry::parse(&json) {
                Ok(parsed) => parsed,
                Err(error) => {
                    self.record_reload_failure(ReloadFailure::Parse);
                    return Err(error).with_context(|| {
                        format!(
                            "replacement credential registry {} rejected",
                            path.display()
                        )
                    });
                }
            };
            if let Err(error) = registry.try_merge(parsed) {
                self.record_reload_failure(ReloadFailure::Invalid);
                return Err(error).with_context(|| format!("merging registry {}", path.display()));
            }
        }
        self.reload_registry(registry)
    }

    /// Validate and atomically adopt an already-parsed replacement.
    pub fn reload_registry(&self, registry: Registry) -> Result<u64> {
        if let Err(error) = self.validate(&registry) {
            self.record_reload_failure(ReloadFailure::Invalid);
            return Err(error);
        }

        let (revision, entries) = {
            let mut current = self.write_snapshot();
            let revision = current.revision.saturating_add(1);
            *current = RegistrySnapshot { revision, registry };
            (revision, current.registry.len())
        };
        self.sink.record(&AuthEvent::RegistryReload {
            applied: true,
            revision,
            entries,
            failure: None,
        });
        Ok(revision)
    }

    /// Everything a candidate snapshot must satisfy before adoption: the
    /// shared structural rules, plus this verifier's own reservations.
    fn validate(&self, registry: &Registry) -> Result<()> {
        validate_registry(self.required, registry)?;
        if let Some((section, key, subject)) =
            registry.reserved_subject_violation(&self.reserved_subjects)
        {
            bail!(
                "replacement registry `{section}` entry `{key}` claims the reserved subject \
                 `{subject}`, which this service presents on its own behalf"
            );
        }
        Ok(())
    }

    fn record_reload_failure(&self, failure: ReloadFailure) {
        let current = self.read_snapshot();
        self.sink.record(&AuthEvent::RegistryReload {
            applied: false,
            revision: current.revision,
            entries: current.registry.len(),
            failure: Some(failure),
        });
    }

    fn read_snapshot(&self) -> std::sync::RwLockReadGuard<'_, RegistrySnapshot> {
        self.snapshot
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_snapshot(&self) -> std::sync::RwLockWriteGuard<'_, RegistrySnapshot> {
        self.snapshot
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn denied(&self, reason: AuthorizationReason) -> AuthError {
        self.sink.record(&AuthEvent::AuthorizationDecision {
            decision: AuthorizationDecision::Deny,
            reason,
            subject: None,
            resource: None,
            needed: None,
        });
        AuthError::Unauthenticated
    }
}

impl Verifier for ReloadableRoleMapVerifier {
    type Principal = AuditedRoleMapPrincipal;

    fn authenticate(&self, headers: &HeaderMap) -> std::result::Result<Self::Principal, AuthError> {
        let principal = match (self.required, bearer_token(headers)) {
            (false, None) => RoleMapPrincipal::Open,
            // Bearer secrets resolve against `tokens` only. A secret that
            // happens to be a valid email must not reach an identity entry
            // (#2678, R1) — presenting a string is not proving an identity.
            (_, Some(token)) => self
                .lookup_secret(token)
                .map(RoleMapPrincipal::Token)
                .ok_or_else(|| self.denied(AuthorizationReason::UnknownBearer))?,
            (true, None) => return Err(self.denied(AuthorizationReason::MissingBearer)),
        };
        Ok(AuditedRoleMapPrincipal {
            inner: principal,
            sink: Arc::clone(&self.sink),
        })
    }

    fn required(&self) -> bool {
        self.required
    }
}

fn validate_registry(required: bool, registry: &Registry) -> Result<()> {
    if required && registry.is_empty() {
        bail!("auth required but replacement registry is empty");
    }
    validate_entries(&registry.tokens, "token")?;
    validate_entries(&registry.identities, "identity")?;
    // An identity key is an email an identity provider vouched for. Rejecting
    // anything else keeps a bearer secret pasted into the wrong section from
    // being adopted as a grant nobody can trace back to a person (#2678, R2).
    for identity in registry.identities.keys() {
        if !identity.contains('@') {
            bail!("replacement registry identity key is not an email address");
        }
    }
    Ok(())
}

fn validate_entries(entries: &HashMap<String, TokenClaims>, kind: &str) -> Result<()> {
    for (key, claims) in entries {
        if key.trim().is_empty() {
            bail!("replacement registry contains an empty {kind} key");
        }
        if claims.subject.trim().is_empty() {
            bail!("replacement registry contains an empty subject");
        }
        if claims
            .roles
            .keys()
            .any(|resource| resource.trim().is_empty())
        {
            bail!("replacement registry contains an empty resource key");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
