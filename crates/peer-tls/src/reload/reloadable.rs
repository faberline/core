use std::fmt;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use rustls::pki_types::CertificateDer;
use rustls::{ClientConfig, ServerConfig};

use super::configs::{build_configs, dedupe};
use super::{MaterialSource, TlsReloadStatus, TlsRuntimeProfile};
use crate::material::{validate, Rejection, ValidatedMaterial};

/// One activated generation.
struct Activation {
    generation: u64,
    material: ValidatedMaterial,
    server: Arc<ServerConfig>,
    client: Arc<ClientConfig>,
    trust_anchors: usize,
}

struct State {
    active: Option<Activation>,
    /// Anchors from the generation before the active one, kept trusted until the
    /// certificate controller confirms the fleet activated the new leaf (R5).
    retiring: Vec<CertificateDer<'static>>,
    accepted: u64,
    rejected: u64,
    last_error: Option<Rejection>,
}

/// TLS material that can be replaced while the process keeps serving.
#[derive(Clone)]
pub struct ReloadableTls {
    inner: Arc<Inner>,
}

struct Inner {
    profile: TlsRuntimeProfile,
    source: Arc<dyn MaterialSource>,
    state: RwLock<State>,
}

impl fmt::Debug for ReloadableTls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = self.status();
        f.debug_struct("ReloadableTls")
            .field("generation", &status.generation)
            .field("serving", &status.serving)
            .finish_non_exhaustive()
    }
}

impl ReloadableTls {
    /// Load and activate material now, refusing to exist without it.
    ///
    /// This is the production constructor and the whole of R7's startup half: a
    /// service that requires TLS and cannot prove an identity has nothing useful
    /// to do next, and starting anyway would publish a listener that either
    /// serves plaintext or fails every handshake while readiness says otherwise.
    pub fn required(
        profile: TlsRuntimeProfile,
        source: Arc<dyn MaterialSource>,
    ) -> Result<Self, Rejection> {
        Self::required_at(profile, source, SystemTime::now())
    }

    /// [`Self::required`] against an explicit instant, for deterministic tests.
    pub fn required_at(
        profile: TlsRuntimeProfile,
        source: Arc<dyn MaterialSource>,
        now: SystemTime,
    ) -> Result<Self, Rejection> {
        let tls = Self::pending(profile, source);
        tls.reload_at(now)?;
        Ok(tls)
    }

    /// Construct without material, for a runtime that expects its first
    /// projection to arrive later. Nothing is served until a reload succeeds.
    pub fn pending(profile: TlsRuntimeProfile, source: Arc<dyn MaterialSource>) -> Self {
        Self {
            inner: Arc::new(Inner {
                profile,
                source,
                state: RwLock::new(State {
                    active: None,
                    retiring: Vec::new(),
                    accepted: 0,
                    rejected: 0,
                    last_error: None,
                }),
            }),
        }
    }

    pub fn profile(&self) -> &TlsRuntimeProfile {
        &self.inner.profile
    }

    /// Read, validate, and activate the current material.
    pub fn reload(&self) -> Result<u64, Rejection> {
        self.reload_at(SystemTime::now())
    }

    /// [`Self::reload`] against an explicit instant.
    pub fn reload_at(&self, now: SystemTime) -> Result<u64, Rejection> {
        match self.try_activate(now) {
            Ok(generation) => Ok(generation),
            Err(rejection) => {
                self.record_rejection(rejection.clone());
                Err(rejection)
            }
        }
    }

    /// Everything that can fail, done before the write lock is taken.
    fn try_activate(&self, now: SystemTime) -> Result<u64, Rejection> {
        let pem = self.inner.source.load()?;
        let material = validate(&pem, &self.inner.profile.identity, now)?;

        // The anchors the previous generation was validated against stay trusted
        // across the swap: during an issuer rotation the fleet does not turn
        // over in one instant, and dropping the old root the moment this member
        // has a new leaf is how a rotation becomes a partition (R5).
        let carried = {
            let state = self.read_state();
            let mut carried = state.retiring.clone();
            if let Some(active) = &state.active {
                if active.material.fingerprint() == material.fingerprint() {
                    // Nothing changed. Re-activating would churn the generation
                    // for every poll tick and make "generation" useless as a
                    // signal that the leaf moved.
                    return Ok(active.generation);
                }
                carried.extend(active.material.trust_anchors().iter().cloned());
            }
            carried
        };

        let (server, client, trust_anchors) =
            build_configs(&self.inner.profile, &material, &carried)?;

        let mut state = self.write_state();
        let generation = state
            .active
            .as_ref()
            .map(|active| active.generation)
            .unwrap_or(0)
            .saturating_add(1);
        let previous_anchors = state
            .active
            .as_ref()
            .map(|active| active.material.trust_anchors().to_vec())
            .unwrap_or_default();
        state.retiring = dedupe(state.retiring.drain(..).chain(previous_anchors));
        state.active = Some(Activation {
            generation,
            material,
            server,
            client,
            trust_anchors,
        });
        state.accepted = state.accepted.saturating_add(1);
        state.last_error = None;
        Ok(generation)
    }

    /// Drop the trust carried from earlier generations.
    ///
    /// Called once the certificate controller has observed every member running
    /// `generation`. Returns `false` — and changes nothing — when the argument
    /// names some other generation, because a retirement racing an activation
    /// would otherwise retire trust the fleet has not finished adopting (R5).
    pub fn retire_previous_trust(&self, generation: u64) -> bool {
        self.retire_previous_trust_at(generation, SystemTime::now())
    }

    /// [`Self::retire_previous_trust`] against an explicit instant.
    pub fn retire_previous_trust_at(&self, generation: u64, now: SystemTime) -> bool {
        let mut state = self.write_state();
        let Some(active) = &state.active else {
            return false;
        };
        if active.generation != generation {
            return false;
        }
        if state.retiring.is_empty() {
            return true;
        }
        let Ok((server, client, trust_anchors)) =
            build_configs(&self.inner.profile, &active.material, &[])
        else {
            return false;
        };
        let _ = now;
        state.retiring.clear();
        if let Some(active) = state.active.as_mut() {
            active.server = server;
            active.client = client;
            active.trust_anchors = trust_anchors;
        }
        true
    }

    /// The server configuration a handshake accepted right now should use, or
    /// `None` when nothing valid is active.
    pub fn server_config(&self) -> Option<Arc<ServerConfig>> {
        self.server_config_at(SystemTime::now())
    }

    /// [`Self::server_config`] against an explicit instant.
    pub fn server_config_at(&self, now: SystemTime) -> Option<Arc<ServerConfig>> {
        let state = self.read_state();
        let active = state.active.as_ref()?;
        active
            .material
            .is_valid_at(now)
            .then(|| Arc::clone(&active.server))
    }

    /// The client configuration for dialing a peer, or `None` when nothing valid
    /// is active.
    pub fn client_config(&self) -> Option<Arc<ClientConfig>> {
        self.client_config_at(SystemTime::now())
    }

    /// [`Self::client_config`] against an explicit instant.
    pub fn client_config_at(&self, now: SystemTime) -> Option<Arc<ClientConfig>> {
        let state = self.read_state();
        let active = state.active.as_ref()?;
        active
            .material
            .is_valid_at(now)
            .then(|| Arc::clone(&active.client))
    }

    /// The generation currently activated; `0` before the first activation.
    pub fn generation(&self) -> u64 {
        self.read_state()
            .active
            .as_ref()
            .map(|active| active.generation)
            .unwrap_or(0)
    }

    /// Fingerprint of the active leaf.
    pub fn fingerprint(&self) -> Option<String> {
        self.read_state()
            .active
            .as_ref()
            .map(|active| active.material.fingerprint().to_string())
    }

    pub fn status(&self) -> TlsReloadStatus {
        self.status_at(SystemTime::now())
    }

    /// [`Self::status`] against an explicit instant.
    pub fn status_at(&self, now: SystemTime) -> TlsReloadStatus {
        let state = self.read_state();
        let active = state.active.as_ref();
        TlsReloadStatus {
            generation: active.map(|a| a.generation).unwrap_or(0),
            fingerprint: active.map(|a| a.material.fingerprint().to_string()),
            seconds_to_expiry: active.map(|a| a.material.seconds_to_expiry(now)),
            accepted_reloads: state.accepted,
            rejected_reloads: state.rejected,
            last_error_reason: state.last_error.as_ref().map(|e| e.reason.as_str()),
            last_error: state.last_error.as_ref().map(|e| e.detail.clone()),
            trust_anchors: active.map(|a| a.trust_anchors).unwrap_or(0),
            retiring_trust_anchors: state.retiring.len(),
            serving: active.is_some_and(|a| a.material.is_valid_at(now)),
        }
    }

    fn record_rejection(&self, rejection: Rejection) {
        let mut state = self.write_state();
        state.rejected = state.rejected.saturating_add(1);
        state.last_error = Some(rejection);
    }

    fn read_state(&self) -> std::sync::RwLockReadGuard<'_, State> {
        self.inner
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_state(&self) -> std::sync::RwLockWriteGuard<'_, State> {
        self.inner
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
